const fs=require('fs');const path=require('path');const {chromium}=require('playwright');
(async()=>{
 const f=JSON.parse(fs.readFileSync(0,'utf8'));const browser=await chromium.launch({headless:true,args:['--no-sandbox']});const context=await browser.newContext();
 const check=(ok,message)=>{if(!ok)throw new Error(message);};
 const call=async(url,data,token,method='POST')=>{const r=await context.request.fetch(f.base+url,{method,data,headers:token?{Authorization:'Bearer '+token}:{}});return {status:r.status(),headers:r.headers(),body:await r.json()};};
 const login=async(username='admin')=>(await call('/api/auth/login',{username,password:f.password})).body.data;
 const endpoint='/api/diagnostics/support-report';const root=await login();
 try {
  check((await call(endpoint,{})).status===401,'anonymous report access');
  for(const role of ['viewer','operator']){
   check((await call('/api/users',{username:'canary_'+role,password:f.password,role},root.token)).body.success,'account creation failed');
   const user=await login('canary_'+role);check((await call(endpoint,{},user.token)).status===403,'non-admin report access');
  }
  const inventory=await call('/api/remote-servers',{name:'CANARY_SERVER_NAME',host:'private-canary-host.invalid',user:'private-canary-user',port:22,auth_method:'password',password:'CANARY_SSH_PASSWORD',memo:'CANARY_MEMO',tags:['CANARY_TAG']},root.token);
  check(inventory.body.success,'inventory setup failed');
  check((await call('/api/auth/register',{username:'canary_manager',password:f.password})).status===201,'manager registration failed');
  const all=(await call('/api/users',undefined,root.token,'GET')).body.data;
  const managerId=all.find(u=>u.username==='canary_manager').id;
  check((await call('/api/users/'+managerId+'/review',{status:'active',memberships:[{kind:'server',scope_id:inventory.body.data.id,role:'admin'}]},root.token)).body.success,'manager review failed');
  const manager=await login('canary_manager');check((await call(endpoint,{},manager.token)).status===403,'scope manager report access');
  const invalid=await context.request.post(f.base+endpoint,{data:{include_secrets:true,path:'/etc/shadow'},headers:{Authorization:'Bearer '+root.token}});
  check(invalid.status()===400,'arbitrary collection options accepted');
  const saved={};for(const file of ['users.json','remote_servers.json'])saved[file]=fs.readFileSync(path.join(f.authDirectory,file),'utf8');
  const page=await context.newPage();const errors=[];const external=[];page.on('pageerror',e=>errors.push(e.message));page.on('request',r=>{if(/^https?:/.test(r.url())&&!r.url().startsWith(f.base+'/'))external.push(r.url());});
  await page.addInitScript(({base,session})=>{if(location.origin===base){sessionStorage.setItem('nabiman_token',session.token);sessionStorage.setItem('nabiman_refresh',session.refresh_token);localStorage.setItem('nabiman_lang','en');}},{base:f.base,session:root});
  await page.goto(f.base+'/#monitoring/diagnostics');
  await page.getByRole('heading',{name:'Support diagnostic report',exact:true}).waitFor();
  const collecting=page.waitForResponse(r=>r.url().endsWith(endpoint)&&r.request().method()==='POST');
  await page.getByRole('button',{name:'Generate diagnostic report',exact:true}).click();
  const response=await collecting;check(response.status()===200&&response.headers()['cache-control']==='no-store','collection status or cache policy failed');
  check((await call(endpoint,{},root.token)).status===429,'collection cooldown missing');
  const report=(await response.json()).data;
  check(report.runtime.executable_sha256===f.binarySha256,'report runtime digest mismatch');
  check(report.accounts.data.total===4&&report.server_inventory.data.registered===1&&(report.recent_jobs.state==='ready'?report.recent_jobs.data.failed===1:report.recent_jobs.data===null),'aggregate counts incorrect');
  check(!JSON.stringify(report).includes('CANARY')&&!JSON.stringify(report).includes('canary_')&&!JSON.stringify(report).includes('private-canary')&&!JSON.stringify(report).includes(f.authDirectory),'private data exported');
  const button=page.getByRole('button',{name:'Download reviewed JSON',exact:true});check(await button.isDisabled(),'download allowed before review');
  const preview=await page.getByLabel('Diagnostic download preview',{exact:true}).textContent();
  await page.getByLabel('I have reviewed the contents and want to download this report',{exact:true}).check();
  const downloading=page.waitForEvent('download');await button.click();const downloaded=await downloading;
  check(fs.readFileSync(await downloaded.path(),'utf8')===preview,'download differs from reviewed preview');
  check(downloaded.suggestedFilename()===`nabiman-support-${report.report_id}.json`,'unsafe or identifying filename');
  for(const [file,content] of Object.entries(saved))check(fs.readFileSync(path.join(f.authDirectory,file),'utf8')===content,'report mutated auth or inventory');
  const inventoryPath=path.join(f.authDirectory,'remote_servers.json');fs.writeFileSync(inventoryPath,'CANARY_MALFORMED_INVENTORY_SECRET');
  try{
   await new Promise(resolve=>setTimeout(resolve,5100));
   await page.getByRole('button',{name:'Generate diagnostic report',exact:true}).click();
   await page.getByText('Some sections could not be collected. Busy, unavailable or too_large sections do not mean healthy or empty.',{exact:true}).waitFor();
   const partial=JSON.parse(await page.getByLabel('Diagnostic download preview',{exact:true}).textContent());
   check(partial.server_inventory.state==='unavailable'&&partial.server_inventory.data===null&&!JSON.stringify(partial).includes('CANARY'),'partial failure leaked raw error or claimed empty inventory');
   check(await page.getByRole('button',{name:'Download reviewed JSON',exact:true}).isDisabled(),'new collection reused old review acknowledgement');
  }finally{fs.writeFileSync(inventoryPath,saved['remote_servers.json']);}
  check(errors.length===0,'Browser exception: '+errors.join('; ').slice(0,500));
  check(external.length===0,'Unexpected external request origins: '+[...new Set(external.map(url=>new URL(url).origin))].join(', '));
  // Mock the external origin: verify the boundary without sending a real report.
  await page.route('https://fix.bebo.kr/**', route=>route.fulfill({contentType:'text/html',body:`<script>
    window.replies=[]; window.parentBlocked=false;
    try { parent.sessionStorage.getItem('nabiman_token'); } catch (_) { window.parentBlocked=true; }
    addEventListener('message',e=>window.replies.push(e.data));
    parent.postMessage({type:'fixhub:ready'},'*');
    parent.postMessage({type:'fixhub:pick'},'*');
    parent.postMessage({type:'fixhub:goto',url:'https://example.invalid/'},'*');
  </script>`}));
  check(await page.locator('[data-nabiman-feedback-panel]').count()===0,'feedback loaded before opt-in');
  await page.getByRole('button',{name:'Feedback (external)',exact:true}).click();
  const iframe=page.locator('[data-nabiman-feedback-panel] iframe'); await iframe.waitFor();
  check(await iframe.getAttribute('referrerpolicy')==='no-referrer','feedback referrer disclosure');
  const isolated=await (await iframe.elementHandle()).contentFrame();
  await isolated.waitForFunction(()=>window.replies.length>=2);
  const boundary=await isolated.evaluate(()=>({blocked:window.parentBlocked,replies:window.replies}));
  check(boundary.blocked,'external frame accessed parent credentials');
  check(JSON.stringify(boundary.replies.find(r=>r.type==='fixhub:context').page)===JSON.stringify({page:'/',query:'',page_title:'NabiMan',context:{}}),'feedback context was not fixed public metadata');
  check(boundary.replies.some(r=>r.type==='fixhub:picked'&&r.selector===null&&r.text===null),'external DOM capture allowed');
  check(page.url()===f.base+'/#monitoring/diagnostics','external navigation was allowed');
  check(await page.locator('script[src^="https://fix.bebo.kr"]').count()===0,'third-party code loaded in parent');
  await page.evaluate(()=>{sessionStorage.removeItem('nabiman_token');window.dispatchEvent(new Event('nabiman_auth_changed'));});
  check(await page.locator('[data-nabiman-feedback-panel], [data-nabiman-feedback-launcher]').count()===0,'logout retained feedback frame');
  console.log('PASS: administrator-only report, exact runtime and aggregate counts, secret canaries excluded, reviewed download equality, no automatic upload, cooldown, read-only stores and honest partial failure.');
 }finally{await browser.close();}
})().catch(e=>{console.error(e.message);process.exit(1);});
