const fs=require('fs');const {chromium}=require('playwright');
const { autoConfirm, answerConfirm } = require('./confirm-modal');
(async()=>{
 const f=JSON.parse(fs.readFileSync(0,'utf8'));const browser=await chromium.launch({headless:true,args:['--no-sandbox']});const context=await browser.newContext();
 const check=(value,message)=>{if(!value)throw new Error(message);};
 const call=async(path,data,token,method='POST')=>{const r=await context.request.fetch(f.base+path,{method,data,headers:token?{Authorization:'Bearer '+token,'X-User':'forged-super-admin'}:{}});return {status:r.status(),body:await r.json(),headers:r.headers()};};
 const login=async(username='admin')=>(await call('/api/auth/login',{username,password:f.password})).body.data.token;
 const root=await login();const get=async(path,token=root)=>call(path,undefined,token,'GET');
 const save=async(path,snapshot,change,reason,token=root)=>{const {revision,...settings}=snapshot;return call(path,{...settings,...change,expected_revision:revision,change_reason:reason},token,'PUT');};
 try {
  if(!f.resume){
   const server=(await call('/api/remote-servers',{name:'History fixture',host:'127.0.0.1',port:1,user:'fixture'},root)).body.data.id;
   const path='/api/scope-settings/server/'+server;
   const ids={};
   for(const name of ['manager','member']){
    check((await call('/api/auth/register',{username:name,password:f.password})).status===201,'registration failed');
    ids[name]=(await get('/api/users')).body.data.find(u=>u.username===name).id;
    check((await call('/api/users/'+ids[name]+'/review',{status:'active',memberships:[{kind:'server',scope_id:server,role:name==='manager'?'admin':'viewer'}]},root)).status===200,'review failed');
   }
   const manager=await login('manager');const member=await login('member');
   const other=(await call('/api/remote-servers',{name:'Other scope',host:'127.0.0.1',port:2,user:'fixture'},root)).body.data.id;
   check((await get('/api/scope-settings/server/'+other+'/history',manager)).status===403,'manager reads another scope history');
   const initial=(await get(path)).body.data;
   check(initial.revision.startsWith('legacy:'),'missing legacy revision');
   check((await get(path+'/history',member)).status===403,'ordinary member reads administrator history');
   check((await call(path,{contact_email:'first@example.test'},manager,'PUT')).status===428,'unconditional write accepted');
   check((await call(path,{expected_revision:initial.revision},manager,'PUT')).status===400,'missing reason accepted');
   check((await call(path,{expected_revision:initial.revision,change_reason:'Forged metadata test',history:[],actor:'forged'},manager,'PUT')).status===400,'caller-supplied history metadata accepted');
   const page=await context.newPage();const errors=[];page.on('pageerror',e=>errors.push(e.message));page.on('dialog',d=>d.accept());await autoConfirm(page);
   await page.addInitScript(({base,token})=>{if(location.origin===base){sessionStorage.setItem('nabiman_token',token);localStorage.setItem('nabiman_lang','ko');}},{base:f.base,token:manager});
   await page.goto(f.base+'/#administration/scopeSettings');
   await page.getByLabel('소속 회원에게 표시할 공지').fill('My unsaved revision');
   await page.getByLabel('변경 사유',{exact:true}).fill('Manager reviewed the notice');
   check((await save(path,initial,{member_notice:'Administrator revision'},'Administrator concurrent change')).status===200,'concurrent initial save failed');
   await page.getByRole('button',{name:'운영 설정 저장',exact:true}).click();
   await page.getByRole('alert').filter({hasText:'작성한 내용은 아직 저장되지 않았습니다'}).waitFor();
   check(await page.getByLabel('소속 회원에게 표시할 공지').inputValue()==='My unsaved revision','conflict discarded unsaved draft');
   check(await page.getByRole('button',{name:'운영 설정 저장',exact:true}).isDisabled(),'stale resubmission permitted');
   check((await get(path)).body.data.member_notice==='Administrator revision','stale form overwrote administrator');
   await page.getByRole('button',{name:'작성 내용 취소하고 최신 설정 불러오기',exact:true}).click();
   await page.waitForFunction(()=>document.querySelector('textarea')?.value==='Administrator revision');
   await page.getByLabel('소속 회원에게 표시할 공지').fill('Reviewed manager revision');
   await page.getByRole('button',{name:'운영 설정 저장',exact:true}).click();
   await page.getByRole('status').filter({hasText:'소속 운영 설정을 저장했습니다'}).waitFor();
   const history=(await get(path+'/history',manager)).body.data;
   check(history.length===2&&history[1].actor==='admin'&&history[0].actor_id===ids.manager&&history[0].actor==='manager','history actor or rejected-change accounting failed');
   check(history[0].fields.some(x=>x.field==='member_notice'&&x.before==='Administrator revision'&&x.after==='Reviewed manager revision'),'history lacks exact before and after');
   await page.locator('summary').filter({hasText:'Manager reviewed the notice'}).click();
   await page.getByRole('cell',{name:'Reviewed manager revision',exact:true}).waitFor();
   const snapshot=(await get(path)).body.data;
   const race=await Promise.all([save(path,snapshot,{member_notice:'Race A'},'Concurrent writer A'),save(path,snapshot,{member_notice:'Race B'},'Concurrent writer B')]);
   check(race.map(r=>r.status).sort().join(',')==='200,409','concurrent writes did not choose exactly one winner');
   const winner=(await get(path)).body.data;
   check((await save(path,winner,{member_notice:snapshot.member_notice},'Return to previous content')).status===200,'return to old value failed');
   check((await save(path,snapshot,{member_notice:'Stale ABA'},'Old version replay')).status===409,'same-content ABA accepted stale revision');
   check((await get(path+'/history')).body.data.length===4,'failed writes entered history');
   check(errors.length===0,'browser errors');
   process.stdout.write(JSON.stringify({path,revision:(await get(path)).body.data.revision}));
  }else{
   const current=await get(f.resume.path);check(current.body.data.revision===f.resume.revision,'revision changed after restart');
   const history=await get(f.resume.path+'/history');check(history.body.data.length===4&&history.headers['cache-control']==='no-store','history did not survive restart');
   check(current.headers['cache-control']==='no-store','policy snapshot is cacheable');
   process.stdout.write(JSON.stringify({passed:true}));
  }
 }finally{await browser.close();}
})().catch(e=>{console.error(e.message);process.exit(1);});
