const fs=require('fs');const {chromium}=require('playwright');
(async()=>{const f=JSON.parse(fs.readFileSync(0,'utf8'));const browser=await chromium.launch({headless:true,executablePath:f.chromiumPath,args:['--no-sandbox']});try{
 const context=await browser.newContext();await context.addInitScript(token=>{sessionStorage.setItem('nabiman_token',token);localStorage.setItem('nabiman_lang','en');},f.token);
 const page=await context.newPage();const errors=[];page.on('pageerror',e=>errors.push(e.message));await page.goto(f.base+'/#system/cron');await page.getByRole('heading',{name:'Scheduled Tasks (Cron)',exact:true}).waitFor();
 await page.getByRole('button',{name:'Add Job',exact:true}).click();await page.getByRole('button',{name:'Read current crontab for review',exact:true}).click();await page.getByLabel('Reviewed existing crontab (read only)',{exact:true}).waitFor();
 await page.getByLabel('Schedule',{exact:true}).fill('0 2 * * *');await page.getByLabel('Command',{exact:true}).fill('/bin/echo nb-cron-browser-fixture');
 const add=page.getByRole('button',{name:'Add',exact:true});if(!await add.isDisabled())throw Error('Missing execution review');await page.getByRole('checkbox').check();
 let response=page.waitForResponse(r=>r.url().endsWith('/api/cron/add')&&r.request().method()==='POST');await add.click();let r=await response;if(r.status()!==200||!(await r.json()).success)throw Error('Cron create failed');if(!/^[a-f0-9]{64}$/.test(r.request().postDataJSON().expected_revision))throw Error('No reviewed revision');
 await page.getByRole('row').filter({hasText:'nb-cron-browser-fixture'}).getByRole('button',{name:'Edit',exact:true}).click();
 await page.getByRole('button',{name:'Read current crontab for review',exact:true}).click();await page.getByLabel('Reviewed existing crontab (read only)',{exact:true}).waitFor();
 await page.getByLabel('Schedule',{exact:true}).fill('@hourly');await page.getByLabel('Command',{exact:true}).fill('/bin/echo nb-cron-browser-edited');
 const save=page.getByRole('button',{name:'Save',exact:true});if(!await save.isDisabled())throw Error('Missing edit review');await page.getByRole('checkbox').check();
 response=page.waitForResponse(r=>r.url().endsWith('/api/cron/update')&&r.request().method()==='POST');await save.click();r=await response;
 if(r.status()!==200||!r.request().postDataJSON().expected_revision)throw Error('Unverified edit');
 await page.getByRole('row').filter({hasText:'nb-cron-browser-edited'}).getByRole('button',{name:'Delete',exact:true}).click();const form=page.locator('form').filter({hasText:'nb-cron-browser-edited'});if(!await form.getByRole('button',{name:'Delete',exact:true}).isDisabled())throw Error('Missing deletion review');await form.getByRole('checkbox').check();
 response=page.waitForResponse(r=>r.url().endsWith('/api/cron/delete')&&r.request().method()==='POST');await form.getByRole('button',{name:'Delete',exact:true}).click();r=await response;if(r.status()!==200||!(await r.json()).success)throw Error('Cron delete failed');if(errors.length)throw Error('Browser errors');
 if(f.root){
  if(!/^\/tmp\/nb-os-account-root-[^/]+$/.test(f.root))throw Error('Invalid private fixture root');
  const cp=require('child_process'),args=[f.root,'/usr/bin/crontab','-u','root'];
  const read=()=>cp.execFileSync('/usr/sbin/chroot',args.concat('-l'),{encoding:'utf8'});
  const install=text=>cp.execFileSync('/usr/sbin/chroot',args.concat('-'),{input:text});
  const original=read(),seed='# retained context\nSHELL=/bin/sh\n@reboot /bin/true\nMAILTO=""\n*\t* * * * /bin/echo fixture\n# last comment\n';
  try{
   install(seed);await page.reload();await page.getByRole('row').filter({hasText:'/bin/echo fixture'}).getByRole('button',{name:'Delete',exact:true}).click();
   const reviewed=page.locator('form').filter({hasText:'/bin/echo fixture'});await reviewed.getByRole('checkbox').check();
   const changed=seed.replace('*\t* * * * /bin/echo fixture','0 2 * * * /bin/echo external\n*\t* * * * /bin/echo fixture');install(changed);
   response=page.waitForResponse(r=>r.url().endsWith('/api/cron/delete')&&r.request().method()==='POST');await reviewed.getByRole('button',{name:'Delete',exact:true}).click();r=await response;
   if(r.status()!==409||read()!==changed)throw Error('External insertion was not preserved');
  }finally{install(original);}
 }
 if(errors.length)throw Error('Browser errors');
 process.stdout.write(JSON.stringify({passed:true,review_and_confirmation_required:true,revision_submitted:true,real_create_edit_delete:true,external_insert_rejected:f.root?true:null,browser_errors:0}));
}finally{await browser.close();}})().catch(()=>{process.stderr.write('Cron browser fixture failed');process.exit(1);});
