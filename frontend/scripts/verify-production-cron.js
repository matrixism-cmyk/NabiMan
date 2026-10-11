const fs=require('fs');const {chromium}=require('playwright');
(async()=>{const f=JSON.parse(fs.readFileSync(0,'utf8'));const browser=await chromium.launch({headless:true,executablePath:f.chromiumPath,args:['--no-sandbox']});try{
 const context=await browser.newContext();await context.addInitScript(token=>{sessionStorage.setItem('nabiman_token',token);localStorage.setItem('nabiman_lang','en');},f.token);
 const page=await context.newPage();let writes=0;const errors=[];page.on('pageerror',e=>errors.push(e.message));page.on('request',r=>{if(r.url().includes('/api/cron')&&r.method()!=='GET')writes++;});
 await page.goto(f.base+'/#system/cron');await page.getByRole('heading',{name:'Scheduled Tasks (Cron)',exact:true}).waitFor();
 await page.getByRole('button',{name:'Add Job',exact:true}).click();await page.getByRole('button',{name:'Read current crontab for review',exact:true}).click();await page.getByLabel('Reviewed existing crontab (read only)',{exact:true}).waitFor();
 if(!await page.getByRole('button',{name:'Add',exact:true}).isDisabled())throw Error('Missing review confirmation');await page.getByRole('button',{name:'Cancel',exact:true}).last().click();
 await page.getByRole('button',{name:'Edit',exact:true}).first().click();await page.getByLabel('Job before change',{exact:true}).waitFor();
 await page.getByRole('button',{name:'Read current crontab for review',exact:true}).click();await page.getByLabel('Reviewed existing crontab (read only)',{exact:true}).waitFor();
 if(!await page.getByRole('button',{name:'Save',exact:true}).isDisabled())throw Error('Missing edit confirmation');await page.getByLabel('Job to save',{exact:true}).waitFor();
 await page.getByRole('button',{name:'Cancel',exact:true}).last().click();
 await page.screenshot({path:f.screenshot,fullPage:true});if(writes||errors.length)throw Error('Unexpected mutation or browser error');
 process.stdout.write(JSON.stringify({passed:true,existing_settings_review_available:true,execution_confirmation_required:true,edit_review_available:true,cron_write_requests:0,browser_errors:0}));
}finally{await browser.close();}})().catch(()=>{process.stderr.write('Production cron read-only verification failed');process.exit(1);});
