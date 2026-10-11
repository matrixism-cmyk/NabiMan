const fs=require('fs');const crypto=require('crypto');const {chromium}=require('playwright');
function otp(secret){let bits='';for(const c of secret)bits+='ABCDEFGHIJKLMNOPQRSTUVWXYZ234567'.indexOf(c).toString(2).padStart(5,'0');const bytes=[];for(let i=0;i+8<=bits.length;i+=8)bytes.push(parseInt(bits.slice(i,i+8),2));const counter=Buffer.alloc(8);counter.writeBigUInt64BE(BigInt(Math.floor(Date.now()/30000)));const hash=crypto.createHmac('sha1',Buffer.from(bytes)).update(counter).digest();return String((hash.readUInt32BE(hash[19]&15)&0x7fffffff)%1000000).padStart(6,'0');}
(async()=>{
 const f=JSON.parse(fs.readFileSync(0,'utf8'));const browser=await chromium.launch({headless:true,args:['--no-sandbox']});
 const context=await browser.newContext();const errors=[];
 const call=async(path,data,token,method='POST')=>{const r=await context.request.fetch(f.base+path,{method,data,headers:token?{Authorization:'Bearer '+token}:{}});return {status:r.status(),body:await r.json(),headers:r.headers()};};
 const assert=(ok,why)=>{if(!ok)throw new Error(why);};
 const login=async(username='admin')=>(await call('/api/auth/login',{username,password:f.password})).body.data;
 const factor=async(username,code)=>(await call('/api/auth/2fa/verify',{username,password:f.password,code})).body.data;
 const root=await login();
 const writePolicy=async(path,body,token)=>{const current=await call(path,undefined,token,'GET');return call(path,{...body,expected_revision:current.body.data?.revision||'not-visible',change_reason:'MFA policy verification'},token,'PUT');};
 const policy=async(kind,id,required,token=root.token)=>writePolicy(`/api/scope-settings/${kind}/${id}`,{require_mfa:required},token);
 const me=async(token)=>call('/api/users/me',undefined,token,'GET');
 const status=async(token)=>call('/api/auth/2fa/status',undefined,token,'GET');
 const enroll=async(token)=>{
   const setup=await call('/api/auth/2fa/setup',{password:f.password},token);
   assert(setup.status===200,'setup failed');
   const done=await call('/api/auth/2fa/verify-setup',{code:otp(setup.body.data.secret),recovery_saved:true},token);
   assert(done.status===200,'enrollment confirm failed');return setup.body.data.recovery_codes;
 };
 try{
  if(!f.resume){
   const server=(await call('/api/remote-servers',{name:'Policy fixture',host:'127.0.0.1',port:1,user:'fixture'},root.token)).body.data.id;
   const users={};const sessions={};
   for(const name of ['manager','member','other']){
    assert((await call('/api/auth/register',{username:name,password:f.password})).status===201,'signup failed');
    const list=(await call('/api/users',undefined,root.token,'GET')).body.data;users[name]=list.find(u=>u.username===name).id;
    const memberships=name==='other'?[]:[{kind:'server',scope_id:server,role:name==='manager'?'admin':'viewer'},{kind:'tenant',scope_id:'mfa-tenant',role:name==='manager'?'admin':'viewer'}];
    assert((await call(`/api/users/${users[name]}/review`,{status:'active',memberships},root.token)).status===200,'review failed');
    sessions[name]=await login(name);
   }
   assert((await policy('server',server,true,sessions.member.token)).status===403,'member changed policy');
   assert((await policy('server',server,true,sessions.manager.token)).status===400,'unenrolled manager enabled requirement');
   const managerCodes=await enroll(sessions.manager.token);sessions.manager=await factor('manager',managerCodes[0]);
   assert(sessions.manager.token,'MFA manager login failed');
   const page=await context.newPage();page.on('pageerror',e=>errors.push(e.message));
   await page.addInitScript(({session,base})=>{if(location.origin===base&&!sessionStorage.getItem('nabiman_token')){sessionStorage.setItem('nabiman_token',session.token);sessionStorage.setItem('nabiman_refresh',session.refresh_token);localStorage.setItem('nabiman_lang','ko');}}, {session:sessions.manager,base:f.base});
   await page.goto(f.base+'/#administration/scopeSettings');
   await page.getByRole('heading',{name:'tenant·서버 운영 설정',exact:true}).waitFor();
   await page.getByLabel('관리할 소속').selectOption('server:'+server);
   const memberPage=await context.newPage();memberPage.on('pageerror',e=>errors.push(e.message));
   await memberPage.addInitScript(({session,base})=>{if(location.origin===base){sessionStorage.setItem('nabiman_token',session.token);sessionStorage.setItem('nabiman_refresh',session.refresh_token);localStorage.setItem('nabiman_lang','en');}}, {session:sessions.member,base:f.base});
   await memberPage.goto(f.base+'/#remote/remote');
   await memberPage.getByRole('button',{name:'상태 확인',exact:true}).waitFor();
   await page.getByLabel('소속 회원의 2단계 인증 필수',{exact:true}).check();
   await page.getByLabel('변경 사유',{exact:true}).fill('MFA required for members');
   await page.getByRole('button',{name:'운영 설정 저장',exact:true}).click();
   await page.getByRole('status').filter({hasText:'소속 운영 설정을 저장했습니다'}).waitFor();
   const blocked=await call('/api/remote-servers',undefined,sessions.member.token,'GET');
   assert(blocked.status===403&&blocked.headers['x-nabiman-mfa-required']==='1','existing login not gated');
   const crossOrigin=await context.newPage();await crossOrigin.goto(f.base.replace('127.0.0.1','localhost')+'/signup');
   const exposed=await crossOrigin.evaluate(async({base,token})=>{const anonymous=await fetch(base+'/api/remote-servers');const r=await fetch(base+'/api/remote-servers',{headers:{Authorization:'Bearer '+token}});return [anonymous.status,r.status,r.headers.get('X-Nabiman-MFA-Required')];},{base:f.base,token:sessions.member.token});
   assert(JSON.stringify(exposed)==='[401,403,"1"]','separate frontend origin cannot read authenticated MFA policy denial');await crossOrigin.close();
   const foreign=await context.request.get(f.base+'/api/remote-servers',{headers:{Origin:'https://untrusted.example',Authorization:'Bearer '+root.token}});assert(!foreign.headers()['access-control-allow-origin'],'untrusted origin can read authenticated response');
   const preflight=await context.request.fetch(f.base+'/api/remote-servers',{method:'OPTIONS',headers:{Origin:'https://untrusted.example','Access-Control-Request-Method':'GET','Access-Control-Request-Headers':'authorization'}});assert(preflight.status()===400,'untrusted origin preflight accepted');
   assert((await me(sessions.member.token)).body.data.mfa_blocked,'me lacks enrollment requirement');
   assert(!(await me(sessions.other.token)).body.data.mfa_blocked,'unrelated member gated');
   assert((await call('/api/remote-servers',undefined,sessions.manager.token,'GET')).status===200,'verified manager gated');
   await memberPage.getByRole('button',{name:'상태 확인',exact:true}).click();
   await memberPage.getByText('Your organization requires two-step verification before you can access the service.',{exact:true}).waitFor();
   await memberPage.getByRole('button',{name:'Set up authenticator',exact:true}).waitFor();
   assert(await memberPage.locator('.terminal-window').count()===0,'terminal visible in enrollment gate');
   assert((await policy('tenant','mfa-tenant',true,sessions.manager.token)).status===200,'tenant policy failed');
   assert((await policy('server',server,false,sessions.manager.token)).status===200,'server policy removal failed');
   assert((await status(sessions.member.token)).body.data.required,'weaker scope bypassed stronger policy');
   assert((await writePolicy('/api/scope-settings/tenant/mfa-tenant',{contact_email:'support@example.test'},sessions.manager.token)).status===200,'legacy settings update failed');
   assert((await status(sessions.member.token)).body.data.required,'legacy client silently removed MFA policy');
   const restrictedRefresh=(await call('/api/auth/refresh',{refresh_token:sessions.member.refresh_token})).body.data;
   assert((await call('/api/remote-servers',undefined,restrictedRefresh.token,'GET')).status===403,'refresh bypassed enrollment gate');
   const memberCodes=await enroll(sessions.member.token);let member=await factor('member',memberCodes[0]);
   member=(await call('/api/auth/refresh',{refresh_token:member.refresh_token})).body.data;
   assert((await call('/api/remote-servers',undefined,member.token,'GET')).status===200,'enrolled member still blocked');
   assert((await call('/api/auth/2fa/disable',{password:f.password,code:memberCodes[1]},member.token)).status===400,'required MFA disabled');
   assert((await factor('member',memberCodes[1])).token,'denied disable consumed recovery code');
   assert(errors.length===0,'browser errors');
   process.stdout.write(JSON.stringify({server,users,memberCodes,managerCodes}));
  }else{
   const manager=await factor('manager',f.resume.managerCodes[1]);const member=await factor('member',f.resume.memberCodes[2]);
   assert(manager.token&&member.token,'MFA not retained after restart');
   assert((await status(member.token)).body.data.required,'policy lost after restart');
   const raw=fs.readFileSync(f.policyFile,'utf8');
   try{
    fs.writeFileSync(f.policyFile,'invalid-json');
    assert((await call('/api/remote-servers',undefined,member.token,'GET')).status===403,'corrupt policy failed open');
    assert((await me(member.token)).body.data.mfa_blocked,'corrupt policy missing UI gate');
    assert((await status(member.token)).status===503,'corrupt policy treated as optional');
    assert((await call('/api/remote-servers',undefined,root.token,'GET')).status===200,'global recovery access lost');
   }finally{fs.writeFileSync(f.policyFile,raw);}
   assert((await policy('tenant','mfa-tenant',false,manager.token)).status===200,'manager cannot remove own policy');
   assert(!(await status(member.token)).body.data.required,'policy removal not effective');
   assert((await call('/api/auth/2fa/disable',{password:f.password,code:f.resume.memberCodes[3]},member.token)).status===200,'disable after policy removal failed');
   process.stdout.write(JSON.stringify({passed:true}));
  }
 }finally{await browser.close();}
})().catch(e=>{console.error(e.message);process.exit(1);});
