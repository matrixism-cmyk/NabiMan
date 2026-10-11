const fs=require('fs');const {chromium}=require('playwright');
(async()=>{const f=JSON.parse(fs.readFileSync(0,'utf8'));const browser=await chromium.launch({headless:true,executablePath:f.chromiumPath,args:['--no-sandbox']});try{
 const context=await browser.newContext();await context.addInitScript(token=>{sessionStorage.setItem('nabiman_token',token);localStorage.setItem('nabiman_lang','en');},f.token);
 const page=await context.newPage();let writes=0;const errors=[];page.on('pageerror',e=>errors.push(e.message));page.on('request',r=>{if(r.url().includes('/api/accounts')&&r.method()!=='GET')writes++;});
 await page.goto(f.base+'/#security/accounts');await page.getByRole('heading',{name:'Local OS accounts',exact:true}).waitFor();
 const root=page.getByRole('row').filter({has:page.getByRole('cell',{name:'root',exact:true})});await root.waitFor();
 if(await root.getByRole('button').count())throw Error('Protected root has mutation buttons');
 await page.getByRole('button',{name:'Add User',exact:true}).waitFor();
 await page.screenshot({path:f.screenshot,fullPage:true});if(writes||errors.length)throw Error('Unexpected write or browser error');
 process.stdout.write(JSON.stringify({passed:true,local_os_heading:true,root_protected:true,highest_admin_create_control_visible:true,os_write_requests:0,browser_errors:0}));
}finally{await browser.close();}})().catch(()=>{process.stderr.write('Production OS account read-only verification failed');process.exit(1);});
