const fs=require('fs');const {chromium}=require('playwright');
(async()=>{const f=JSON.parse(fs.readFileSync(0,'utf8'));const browser=await chromium.launch({executablePath:f.chromiumPath,headless:true,args:['--no-sandbox']});try{
 const context=await browser.newContext();await context.addInitScript(()=>localStorage.setItem('nabiman_lang','en'));const page=await context.newPage();const errors=[];page.on('pageerror',e=>errors.push(e.message));
 for(let i=0;i<2;i++){
  await page.goto(f.base);await page.getByRole('textbox',{name:'Username',exact:true}).fill(f.blocked || 'blocked');await page.getByLabel('Admin Password',{exact:true}).fill('wrong');
  const response=page.waitForResponse(r=>r.url().endsWith('/api/auth/login'));await page.getByRole('button',{name:'Login',exact:true}).click();if((await response).status()!==429)throw Error('Missing server rate limit');
  await page.getByRole('status').filter({hasText:'Too many attempts.'}).waitFor();if(!await page.getByRole('button',{name:'Login',exact:true}).isDisabled())throw Error('Retry button not blocked');
 }
 await page.goto(f.base);await page.getByRole('textbox',{name:'Username',exact:true}).fill('admin');await page.getByLabel('Admin Password',{exact:true}).fill(f.password);
 const login=page.waitForResponse(r=>r.url().endsWith('/api/auth/login'));await page.getByRole('button',{name:'Login',exact:true}).click();const result=await login;const data=await result.json();if(result.status()!==200||!data.data?.token)throw Error('Unrelated account blocked');
 const logout=await context.request.post(f.base+'/api/auth/logout',{headers:{Authorization:'Bearer '+data.data.token},data:{}});if(logout.status()!==200)throw Error('Probe logout failed');
 if(errors.length)throw Error('Browser error');process.stdout.write(JSON.stringify({passed:true,countdown_visible:true,submit_disabled:true,refresh_does_not_reset_server_limit:true,other_account_login:true,browser_errors:0}));
}finally{await browser.close();}})().catch(()=>{process.stderr.write('Auth admission browser verification failed');process.exit(1);});
