const fs=require('fs');const crypto=require('crypto');const {chromium}=require('playwright');
const { autoConfirm, answerConfirm } = require('./confirm-modal');
function otp(secret){let bits='';for(const c of secret)bits+='ABCDEFGHIJKLMNOPQRSTUVWXYZ234567'.indexOf(c).toString(2).padStart(5,'0');const bytes=[];for(let i=0;i+8<=bits.length;i+=8)bytes.push(parseInt(bits.slice(i,i+8),2));const counter=Buffer.alloc(8);counter.writeBigUInt64BE(BigInt(Math.floor(Date.now()/30000)));const hash=crypto.createHmac('sha1',Buffer.from(bytes)).update(counter).digest();return String((hash.readUInt32BE(hash[19]&15)&0x7fffffff)%1000000).padStart(6,'0');}
(async()=>{
 const f=JSON.parse(fs.readFileSync(0,'utf8'));const browser=await chromium.launch({headless:true,args:['--no-sandbox']});
 const context=await browser.newContext();const errors=[];
 const call=async(path,data,token,method='POST')=>{const r=await context.request.fetch(f.base+path,{method,data,headers:token?{Authorization:'Bearer '+token}:{}});return {status:r.status(),body:await r.json(),headers:r.headers()};};
 const login=async(name='admin')=>(await call('/api/auth/login',{username:name,password:f.password})).body.data;
 const factor=async(code)=>(await call('/api/auth/2fa/verify',{username:'mfamember',password:f.password,code}));
 const root=await login();let page;
 async function open(session){page=await context.newPage();page.setDefaultTimeout(15000);page.on('pageerror',e=>errors.push(e.name));page.on('dialog',d=>d.accept());await autoConfirm(page);await page.addInitScript(value=>{if(location.origin!==value.origin || sessionStorage.getItem('mfa_fixture'))return;sessionStorage.setItem('mfa_fixture','1');sessionStorage.setItem('nabiman_token',value.token);sessionStorage.setItem('nabiman_refresh',value.refresh_token);localStorage.setItem('nabiman_lang','en');},{...session,origin:new URL(f.base).origin});await page.goto(f.base+'/#administration/mfa');await page.getByRole('heading',{name:'Two-step verification',exact:true}).waitFor();}
 async function browserLogin(code){await page.locator('.login-form input[autocomplete="username"]').fill('mfamember');await page.locator('.login-form input[autocomplete="current-password"]').fill(f.password);await page.locator('.login-form button[type="submit"]').click();await page.getByLabel('Authenticator or recovery code',{exact:true}).fill(code);await page.locator('.login-form button[type="submit"]').click();await page.locator('.app-header').waitFor();await page.goto(f.base+'/#administration/mfa');await page.getByText('Two-step verification is enabled',{exact:true}).waitFor();}
 try {
  if(!f.resume){
   if(!(await call('/api/users',{username:'mfamember',password:f.password,role:'viewer'},root.token)).body.success)throw new Error('Fixture member creation failed');
   const first=await login('mfamember'),other=await login('mfamember');await open(first);
   await page.getByLabel('Current Password',{exact:true}).fill(f.password);
   const issued=page.waitForResponse(r=>r.url().endsWith('/api/auth/2fa/setup'));
   await page.getByRole('button',{name:'Set up authenticator',exact:true}).click();
   const response=await issued;if(response.status()!==200 || response.headers()['cache-control']!=='no-store')throw new Error('Enrollment secret response not protected');
   const enrollment=(await response.json()).data;
   await page.getByLabel('Recovery codes',{exact:true}).waitFor();
   if(new Set(enrollment.recovery_codes).size!==10)throw new Error('Recovery codes missing or duplicated');
   await page.waitForFunction(()=>document.querySelector('img[alt="Authenticator setup QR code"]')?.naturalWidth>0);
   if(!await page.getByRole('button',{name:'Confirm and enable',exact:true}).isDisabled())throw new Error('Recovery save acknowledgement missing');
   const downloading=page.waitForEvent('download');
   await page.getByRole('button',{name:'Download recovery codes',exact:true}).click();
   const download=await downloading;const downloaded=fs.readFileSync(await download.path(),'utf8');
   if(!enrollment.recovery_codes.every(code=>downloaded.includes(code)))throw new Error('Recovery download omitted codes');
   const raw=fs.readFileSync(f.authFile,'utf8');if(enrollment.recovery_codes.some(c=>raw.includes(c)))throw new Error('Plaintext recovery code persisted');
   const all=(await call('/api/users',undefined,root.token,'GET')).body;
   if(JSON.stringify(all).includes(enrollment.secret)||JSON.stringify(all).includes('recovery_hashes'))throw new Error('MFA secrets exposed by member API');
   await page.getByLabel('I have saved my recovery codes securely',{exact:true}).check();
   const code=otp(enrollment.secret);await page.getByLabel('Authenticator or recovery code',{exact:true}).fill(code);
   await page.getByRole('button',{name:'Confirm and enable',exact:true}).click();await page.locator('.login-form').waitFor();
   for(const token of [first.token,other.token])if((await call('/api/users/me',undefined,token,'GET')).status!==401)throw new Error('Pre-MFA login survived enrollment');
   if((await factor(code)).status!==401)throw new Error('Enrollment OTP reused for login');
   await browserLogin(enrollment.recovery_codes[0]);
   await page.getByText('Recovery codes remaining: 9',{exact:true}).waitFor();
   const stored=JSON.parse(fs.readFileSync(f.authFile,'utf8')).find(u=>u.username==='mfamember');
   if(!stored.totp_enabled || stored.mfa.recovery_hashes.length!==9 || stored.mfa.last_used_step===null)throw new Error('Factor consumption not durable');
   if(errors.length)throw new Error('Browser errors');
   process.stdout.write(JSON.stringify({codes:enrollment.recovery_codes,id:stored.id}));
  }else{
   if((await factor(f.resume.codes[0])).status!==401)throw new Error('Consumed recovery code revived after restart');
   const active=await factor(f.resume.codes[1]);if(active.status!==200)throw new Error('Unused recovery code did not survive restart');
   const other=await factor(f.resume.codes[2]);if(other.status!==200)throw new Error('Second independent MFA login failed');
   await open(active.body.data);
   await page.getByLabel('Current Password',{exact:true}).fill(f.password);await page.getByLabel('Authenticator or recovery code',{exact:true}).fill(f.resume.codes[3]);
   const regenerated=page.waitForResponse(r=>r.url().endsWith('/api/auth/2fa/recovery-codes'));
   await page.getByRole('button',{name:'Regenerate recovery codes',exact:true}).click();
   const next=(await (await regenerated).json()).data.recovery_codes;
   await page.getByLabel('Recovery codes',{exact:true}).waitFor();
   if((await call('/api/users/me',undefined,other.body.data.token,'GET')).status!==401)throw new Error('Recovery regeneration left another login active');
   if((await call('/api/users/me',undefined,active.body.data.token,'GET')).status!==200)throw new Error('Current login cannot save regenerated codes');
   if((await factor(f.resume.codes[4])).status!==401)throw new Error('Old recovery codes survived regeneration');
   await page.getByLabel('Current Password',{exact:true}).fill(f.password);await page.getByLabel('Authenticator or recovery code',{exact:true}).fill(next[0]);
   await page.getByRole('button',{name:'Disable two-step verification',exact:true}).click();await page.locator('.login-form').waitFor();
   const disabled=await login('mfamember');if(!disabled.token || disabled.requires_2fa)throw new Error('MFA disable failed');
   const stored=JSON.parse(fs.readFileSync(f.authFile,'utf8')).find(u=>u.username==='mfamember');
   if(stored.id!==f.resume.id || stored.totp_secret || stored.mfa.recovery_hashes.length || stored.totp_enabled)throw new Error('Disable retained factors or changed account identity');
   let throttled=false;
   for(let i=0;i<8;i++){const attempt=await call('/api/auth/2fa/setup',{password:'incorrect'},disabled.token);if(attempt.status===429){throttled=true;break;}if(attempt.status!==400)throw new Error('Invalid enrollment password unexpectedly accepted');}
   if(!throttled)throw new Error('MFA credential attempts were not throttled');
   if(errors.length)throw new Error('Browser errors');
   process.stdout.write(JSON.stringify({passed:true}));
  }
 }finally{await browser.close();}
})().catch(e=>{console.error(e.message);process.exit(1);});
