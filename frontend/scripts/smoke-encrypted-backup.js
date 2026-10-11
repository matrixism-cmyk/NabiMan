const fs=require('fs');const path=require('path');const {chromium}=require('playwright');
(async()=>{
 const f=JSON.parse(fs.readFileSync(0,'utf8'));const browser=await chromium.launch({headless:true,args:['--no-sandbox']});const password='fixture encrypted passphrase long';
 try{
  const context=await browser.newContext();const api=context.request;const root=(await (await api.post(f.base+'/api/auth/login',{data:{username:'admin',password:f.password}})).json()).data;const headers={Authorization:'Bearer '+root.token};
  await api.post(f.base+'/api/users',{headers,data:{username:'ordinary',password:f.password,role:'viewer'}});
  const viewer=(await (await api.post(f.base+'/api/auth/login',{data:{username:'ordinary',password:f.password}})).json()).data;
  for(const endpoint of ['create-encrypted','restore-encrypted'])if((await api.post(f.base+'/api/backup/'+endpoint,{headers:{Authorization:'Bearer '+viewer.token},data:{}})).status()!==403)throw Error('Non-admin encryption access');
  const page=await context.newPage();const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(token=>{sessionStorage.setItem('nabiman_token',token);localStorage.setItem('nabiman_lang','en');},root.token);
  await page.goto(f.base+'/#management/backup');await page.getByRole('heading',{name:'Encrypted backup · verified restore',exact:true}).waitFor();
  await page.getByLabel('Paths to encrypt (comma separated)',{exact:true}).fill(f.source);
  await page.getByLabel('New backup passphrase',{exact:true}).fill(password);await page.getByLabel('Confirm new backup passphrase',{exact:true}).fill(password);
  const created=page.waitForResponse(r=>r.url().endsWith('/create-encrypted'));await page.getByRole('button',{name:'Create encrypted backup',exact:true}).click();const response=await created;
  if(response.status()!==200||response.headers()['cache-control']!=='no-store')throw Error('Encrypted creation failed');const filename=(await response.json()).data;
  if(await page.getByLabel('New backup passphrase',{exact:true}).inputValue())throw Error('Passphrase retained');
  const artifact=path.join(f.directory,'auth/backups',filename);const original=fs.readFileSync(artifact);
  const restore=async(target,passwordOverride=password,file=filename)=>api.post(f.base+'/api/backup/restore-encrypted',{headers,data:{filename:file,target_dir:path.join(f.directory,target),password:passwordOverride}});
  if((await restore('wrong','wrong fixture password')).status()!==422||fs.existsSync(path.join(f.directory,'wrong')))throw Error('Wrong password published a target');
  for(const [name,data] of [['truncated',original.subarray(0,original.length-1)],['tampered',Buffer.from(original)]]){
   if(name==='tampered')data[data.length-1]^=1;fs.writeFileSync(artifact,data);try{if((await restore(name)).status()!==422||fs.existsSync(path.join(f.directory,name)))throw Error('Invalid ciphertext published target');}finally{fs.writeFileSync(artifact,original);}
  }
  fs.mkdirSync(path.join(f.directory,'existing'));fs.writeFileSync(path.join(f.directory,'existing/canary'),'preserve');
  if((await restore('existing')).status()!==422||fs.readFileSync(path.join(f.directory,'existing/canary'),'utf8')!=='preserve')throw Error('Existing target overwritten');
  fs.writeFileSync(path.join(f.directory,'auth/backups/legacy.enc'),'legacy sentinel');if((await restore('legacy','old password','legacy.enc')).status()!==422||fs.readFileSync(path.join(f.directory,'auth/backups/legacy.enc'),'utf8')!=='legacy sentinel')throw Error('Legacy file changed');
  await page.getByLabel('Encrypted backup file',{exact:true}).selectOption(filename);
  await page.getByLabel('Restore directory that does not exist yet',{exact:true}).fill(path.join(f.directory,'restored'));
  await page.getByLabel('Passphrase for this backup',{exact:true}).fill(password);
  const button=page.getByRole('button',{name:'Verify and restore to new directory',exact:true});if(!await button.isDisabled())throw Error('Restore review missing');
  await page.getByLabel('I will review the restored contents before applying them',{exact:true}).check();await button.click();
  await page.getByText('Verified and restored to the new directory. Review contents before applying.',{exact:true}).waitFor({timeout:30000});
  if(await page.getByLabel('Passphrase for this backup',{exact:true}).inputValue())throw Error('Restore password retained');
  if(fs.statSync(artifact).mode&0o077)throw Error('Public encrypted artifact permissions');
  if(process.env.NABIMAN_BACKUP_SCREENSHOT)await page.screenshot({path:process.env.NABIMAN_BACKUP_SCREENSHOT,fullPage:true});
  if(errors.length)throw Error('Browser exception');process.stdout.write(JSON.stringify({filename}));
 }finally{await browser.close();}
})().catch(e=>{console.error(e.message);process.exit(1);});
