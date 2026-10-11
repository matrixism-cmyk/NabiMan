const fs=require('fs');
const {chromium}=require('playwright');
(async()=>{
  const fixture=JSON.parse(fs.readFileSync(0,'utf8'));
  const browser=await chromium.launch({headless:true,args:['--no-sandbox']});
  const context=await browser.newContext();
  const base=fixture.base;
  const login=async()=> (await (await context.request.post(base+'/api/auth/login', {data:{username:'admin',password:fixture.password}})).json()).data;
  const refresh=async token=>context.request.post(base+'/api/auth/refresh',{data:{refresh_token:token}});
  const me=async token=>(await context.request.get(base+'/api/users/me',{headers:{Authorization:'Bearer '+token}})).status();
  let terminal; let cleanupToken;
  try {
    const initial=await login(), independent=await login();
    cleanupToken=independent.token;
    const page=await context.newPage();page.setDefaultTimeout(15000);
    await page.addInitScript(session=>{
      if (location.origin!==session.origin || sessionStorage.getItem('fixture_initialized')) return;
      sessionStorage.setItem('fixture_initialized','1');
      sessionStorage.setItem('nabiman_token',session.token);sessionStorage.setItem('nabiman_refresh',session.refresh_token);
      localStorage.setItem('nabiman_lang','en');
    },{...initial,origin:new URL(base).origin});
    let rotations=0, expected=initial.refresh_token, lateStatus=true, expireSessions=false;
    let releaseLate; const rotated=new Promise(resolve=>{releaseLate=resolve;});
    const failures=[];
    page.on('pageerror',e=>failures.push(e.name+': '+e.message));
    await page.route('**/api/users/me', async route=>{
      if(rotations===0 && route.request().headers().authorization==='Bearer '+initial.token){await route.fulfill({status:401,json:{success:false,message:'Fixture expired access token'}});}
      else await route.continue();
    });
    await page.route('**/api/server/status',async route=>{
      if(lateStatus){lateStatus=false;await rotated;await route.fulfill({status:401,json:{success:false,message:'Delayed fixture expiry'}});}
      else await route.continue();
    });
    await page.route('**/api/auth/sessions',async route=>{
      if(expireSessions){expireSessions=false;await route.fulfill({status:401,json:{success:false,message:'Fixture next access expiry'}});}
      else await route.continue();
    });
    await page.route('**/api/auth/refresh',async route=>{
      try {
        if(route.request().postDataJSON().refresh_token!==expected) throw new Error('Browser reused an old refresh token');
        const res=await route.fetch();
        if(res.status()===401){await route.fulfill({response:res});return;}
        if(res.status()!==200)throw new Error('Real token rotation failed');
        const body=await res.json();if(body.data.refresh_token===expected)throw new Error('Refresh token did not change');
        expected=body.data.refresh_token;rotations++;
        await route.fulfill({response:res});releaseLate();
      } catch(e){failures.push(e.message);releaseLate();await route.abort();}
    });
    await page.goto(base+'/#administration/loginSessions');
    await page.getByText('Current login',{exact:true}).waitFor();
    await page.waitForFunction(old=>sessionStorage.getItem('nabiman_refresh')!==old,initial.refresh_token);
    await page.waitForLoadState('networkidle');
    if(rotations!==1)throw new Error('Concurrent or delayed 401 caused duplicate rotation');
    if(await page.evaluate(()=>sessionStorage.getItem('nabiman_refresh'))!==expected)throw new Error('Rotated credential not saved');
    expireSessions=true; const beforeNext=expected;
    await page.getByRole('button',{name:'Refresh',exact:true}).click();
    await page.waitForFunction(old=>sessionStorage.getItem('nabiman_refresh')!==old,beforeNext);
    if(rotations!==2)throw new Error('Next expiry did not rotate exactly once');
    const latest=await page.evaluate(()=>({token:sessionStorage.getItem('nabiman_token'),refresh:sessionStorage.getItem('nabiman_refresh')}));
    if(await me(latest.token)!==200)throw new Error('Rotated login unusable');
    terminal=await page.evaluate(async token=>{
      const response=await fetch('/api/terminal/tickets',{method:'POST',headers:{'Content-Type':'application/json',Authorization:'Bearer '+token},body:JSON.stringify({query:''})});
      const issued=await response.json();
      if(!issued.success)throw new Error('Fixture terminal ticket failed');
      return new Promise((resolve,reject)=>{
        const socket=new WebSocket(location.origin.replace(/^http/,'ws')+'/api/terminal?ticket='+issued.data.ticket);
        window.fixtureCloseCode=null;
        socket.onclose=event=>{window.fixtureCloseCode=event.code;};
        socket.onerror=()=>reject(new Error('Fixture terminal connection failed'));
        socket.onmessage=event=>{if(typeof event.data==='string' && event.data.startsWith('\x02SESSION:'))resolve(event.data.slice(9));};
      });
    },latest.token);
    if((await refresh(initial.refresh_token)).status()!==401)throw new Error('Used refresh token accepted');
    await page.waitForFunction(()=>window.fixtureCloseCode===1008);
    const inventory=await (await context.request.get(base+'/api/terminal/sessions',{headers:{Authorization:'Bearer '+independent.token}})).json();
    if(!inventory.data.some(s=>s.id===terminal && s.state==='alive'))throw new Error('Revocation killed the underlying terminal');
    if(await me(latest.token)!==401 || (await refresh(latest.refresh)).status()!==401)throw new Error('Replayed login family survived');
    if(await me(independent.token)!==200)throw new Error('Unrelated login was revoked');
    await page.getByRole('button',{name:'Refresh',exact:true}).click();
    await page.getByText('Your login could not be renewed. Please sign in again.',{exact:true}).waitFor();
    const concurrent=await login();
    const responses=await Promise.all([refresh(concurrent.refresh_token),refresh(concurrent.refresh_token)]);
    if(responses.map(r=>r.status()).sort().join(',')!=='200,401')throw new Error('Concurrent refresh did not have exactly one winner');
    const winner=await responses.find(r=>r.status()===200).json();
    if(await me(winner.data.token)!==401)throw new Error('Concurrent replay did not revoke the winning family');
    if(failures.length)throw new Error('Browser failure: '+failures.join(','));
    console.log('PASS: browser saves rotated credentials, shares delayed/concurrent 401 renewal and uses the new refresh token next; replay and simultaneous redemption revoke only the affected login.');
  }finally{
    try { if(terminal && cleanupToken){const result=await context.request.delete(base+'/api/terminal/sessions/'+encodeURIComponent(terminal),{headers:{Authorization:'Bearer '+cleanupToken}});if(result.status()!==200)throw new Error('Own fixture terminal cleanup failed');} }
    finally { await browser.close(); }
  }
})().catch(e=>{console.error(e.message);process.exit(1);});
