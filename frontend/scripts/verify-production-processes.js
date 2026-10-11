const fs=require('fs');const {chromium}=require('playwright');
(async()=>{const f=JSON.parse(fs.readFileSync(0,'utf8'));const browser=await chromium.launch({headless:true,executablePath:f.chromiumPath,args:['--no-sandbox']});try{
 const context=await browser.newContext();await context.addInitScript(token=>{sessionStorage.setItem('nabiman_token',token);localStorage.setItem('nabiman_lang','en');},f.token);
 const page=await context.newPage();let writes=0;const errors=[];page.on('pageerror',e=>errors.push(e.message));page.on('request',r=>{if(r.url().includes('/api/processes')&&r.method()!=='GET')writes++;});
 await page.goto(f.base+'/#monitoring/processes');await page.getByRole('heading',{name:'Process Monitor',exact:true}).waitFor();await page.getByRole('button',{name:'TERM',exact:true}).first().click();
 await page.getByLabel('Type the displayed PID to confirm the target',{exact:true}).waitFor();if(!await page.getByRole('button',{name:'Send signal to reviewed process',exact:true}).isDisabled())throw Error('Missing target review');await page.getByRole('button',{name:'Cancel',exact:true}).click();
 await page.screenshot({path:f.screenshot,fullPage:true});if(writes||errors.length)throw Error('Unexpected process mutation or browser error');process.stdout.write(JSON.stringify({passed:true,process_inventory_visible:true,target_review_required:true,process_write_requests:0,browser_errors:0}));
}finally{await browser.close();}})().catch(()=>{process.stderr.write('Production process read-only verification failed');process.exit(1);});
