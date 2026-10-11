const fs=require('fs');const {chromium}=require('playwright');
(async()=>{const f=JSON.parse(fs.readFileSync(0,'utf8'));const browser=await chromium.launch({headless:true,executablePath:f.chromiumPath,args:['--no-sandbox']});try{
 const context=await browser.newContext();await context.addInitScript(token=>{sessionStorage.setItem('nabiman_token',token);localStorage.setItem('nabiman_lang','en');},f.token);
 const page=await context.newPage();const errors=[];page.on('pageerror',e=>errors.push(e.message));await page.goto(f.base+'/#security/accounts');await page.getByRole('heading',{name:'Local OS accounts',exact:true}).waitFor();
 await page.getByRole('button',{name:'Add User',exact:true}).click();await page.getByLabel('Username',{exact:true}).fill('nbosbrowser');await page.getByLabel('New Password',{exact:true}).fill('fixture-browser-password-long');
 let response=page.waitForResponse(r=>r.url().endsWith('/api/accounts')&&r.request().method()==='POST');await page.getByRole('button',{name:'Create',exact:true}).click();let r=await response;if(r.status()!==200||!(await r.json()).success)throw Error('Create failed');
 fs.writeFileSync(f.root+'/home/nbosbrowser/canary','retain');
 await page.getByRole('row').filter({hasText:'nbosbrowser'}).getByRole('button',{name:'Delete',exact:true}).click();const confirmation=page.getByLabel('Type the account name to confirm deletion',{exact:true});const form=page.locator('form').filter({has:confirmation});
 if(await form.getByRole('checkbox').isChecked()||!await form.getByRole('button',{name:'Delete',exact:true}).isDisabled())throw Error('Unsafe delete defaults');
 await confirmation.fill('nbosbrowser');response=page.waitForResponse(r=>r.url().endsWith('/api/accounts')&&r.request().method()==='DELETE');await form.getByRole('button',{name:'Delete',exact:true}).click();r=await response;
 if(r.status()!==200||!(await r.json()).success)throw Error('Delete failed');const payload=r.request().postDataJSON();if(payload.remove_home!==false||typeof payload.expected_uid!=='number')throw Error('Missing identity or preservation intent');
 if(fs.readFileSync(f.root+'/home/nbosbrowser/canary','utf8')!=='retain'||errors.length)throw Error('Preservation or browser failure');
 process.stdout.write(JSON.stringify({passed:true,real_create_delete:true,typed_confirmation:true,observed_uid_submitted:true,home_preserved_by_default:true,browser_errors:0}));
}finally{await browser.close();}})().catch(()=>{process.stderr.write('OS account browser fixture failed');process.exit(1);});
