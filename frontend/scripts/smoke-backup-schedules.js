const fs=require('fs');const {chromium}=require('playwright');
(async()=>{
 const f=JSON.parse(fs.readFileSync(0,'utf8'));const browser=await chromium.launch({headless:true,args:['--no-sandbox']});
 try{
  const context=await browser.newContext();const api=context.request;
  const root=(await (await api.post(f.base+'/api/auth/login',{data:{username:'admin',password:f.password}})).json()).data;
  const headers={Authorization:'Bearer '+root.token};
  if((await api.get(f.base+'/api/backup/schedules')).status()!==401)throw Error('anonymous backup access');
  await api.post(f.base+'/api/users',{headers,data:{username:'backup_viewer',password:f.password,role:'viewer'}});
  const viewer=(await (await api.post(f.base+'/api/auth/login',{data:{username:'backup_viewer',password:f.password}})).json()).data;
  if(!viewer || (await api.get(f.base+'/api/backup/schedules',{headers:{Authorization:'Bearer '+viewer.token}})).status()!==403)throw Error('viewer can read backup schedule paths');
  for(const data of [{name:'invalid',paths:['--checkpoint-action=exec=invalid'],cron_expr:'* * * * *'},{name:'invalid',paths:[f.source],cron_expr:'60 * * * *'}])if((await api.post(f.base+'/api/backup/schedules',{headers,data})).status()!==400)throw Error('unsafe schedule accepted');
  const page=await context.newPage();const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(token=>{sessionStorage.setItem('nabiman_token',token);localStorage.setItem('nabiman_lang','en');},root.token);
  await page.goto(f.base+'/#management/backup');
  await page.getByRole('button',{name:'Schedules',exact:true}).click();
  await page.getByRole('button',{name:'Add Schedule',exact:true}).click();
  await page.getByLabel('Schedule Name',{exact:true}).fill('fixture archive');
  await page.getByLabel('Paths (comma separated)',{exact:true}).fill(f.source);
  await page.getByLabel('Cron Expression',{exact:true}).fill('0 0 29 2 *');
  await page.getByRole('button',{name:'Add',exact:true}).click();await page.getByText('Schedule saved',{exact:true}).waitFor();
  const row=page.locator('tr').filter({hasText:'fixture archive'});
  await row.getByRole('button',{name:'Run now',exact:true}).click();
  await row.getByText('Archive created · restore verification required',{exact:true}).waitFor({timeout:20000});
  let rows=(await (await api.get(f.base+'/api/backup/schedules',{headers})).json()).data;const saved=rows.find(r=>r.name==='fixture archive');
  if(!saved.last_result.filename||!saved.last_run)throw Error('missing successful artifact');
  const failed=await api.post(f.base+'/api/backup/schedules',{headers,data:{name:'missing source',paths:[f.source+'/missing'],cron_expr:'0 0 29 2 *'}});const id=(await failed.json()).data.id;
  await api.post(f.base+'/api/backup/schedules/'+id+'/run',{headers});
  await page.getByText('Failed',{exact:true}).waitFor({timeout:20000});
  rows=(await (await api.get(f.base+'/api/backup/schedules',{headers})).json()).data;const failure=rows.find(r=>r.id===id);
  if(failure.last_run!==null||failure.last_result.filename!==null||failure.last_result.state!=='failed')throw Error('failed backup falsely marked successful');
  const auto=(await (await api.post(f.base+'/api/backup/schedules',{headers,data:{name:'automatic fixture',paths:[f.source],cron_expr:'* * * * *',compression:false}})).json()).data;
  let automatic;
  for(let i=0;i<100;i++){
    automatic=(await (await api.get(f.base+'/api/backup/schedules',{headers})).json()).data.find(r=>r.id===auto.id);
    if(automatic.last_result?.state==='succeeded')break;
    await new Promise(resolve=>setTimeout(resolve,100));
  }
  if(automatic.last_result?.state!=='succeeded'||!automatic.last_result.filename.endsWith('.tar')||!automatic.last_slot)throw Error('UTC automatic scheduler did not produce a tar');
  await new Promise(resolve=>setTimeout(resolve,5100));
  const same=(await (await api.get(f.base+'/api/backup/schedules',{headers})).json()).data.find(r=>r.id===auto.id);
  if(same.last_slot===automatic.last_slot&&same.last_result.filename!==automatic.last_result.filename)throw Error('schedule ran twice in the same minute');
  await api.delete(f.base+'/api/backup/schedules/'+auto.id,{headers});
  if(errors.length)throw Error('browser error');
  process.stdout.write(JSON.stringify({id:saved.id,filename:saved.last_result.filename}));
 }finally{await browser.close();}
})().catch(e=>{console.error(e.message);process.exit(1);});
