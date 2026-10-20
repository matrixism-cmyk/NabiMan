const fs = require('fs');
const { chromium } = require('playwright');
const { autoConfirm, answerConfirm } = require('./confirm-modal');
(async () => {
  const fixture = JSON.parse(fs.readFileSync(0, 'utf8'));
  const browser = await chromium.launch({headless:true,args:['--no-sandbox']});
  const client = await browser.newContext();
  const base = fixture.base;
  const request = async (path, token, data, method='POST') => {
    const res = await client.request.fetch(base+path,{method,headers:token?{Authorization:'Bearer '+token}:{},data});
    return {status:res.status(),body:await res.json()};
  };
  const login = async name => (await request('/api/auth/login',null,{username:name,password:fixture.password})).body.data;
  const admin = await login('admin');
  const create = async () => {
    const result = await request('/api/users',admin.token,{username:'sessionviewer',password:fixture.password,role:'viewer'});
    if (!result.body.success) throw new Error('Fixture account creation failed');
    return (await request('/api/users',admin.token,undefined,'GET')).body.data.find(user=>user.username==='sessionviewer');
  };
  const errors=[];
  try {
    const oldUser=await create();
    const first=await login('sessionviewer'), second=await login('sessionviewer');
    if ((await request('/api/users/me',first.refresh_token,undefined,'GET')).status!==401) throw new Error('Refresh token accepted as access credential');
    const renewed=await request('/api/auth/refresh',null,{refresh_token:first.refresh_token});
    if (renewed.status!==200) throw new Error('Valid refresh token rejected at refresh endpoint');
    Object.assign(first,renewed.body.data);
    const page=await client.newPage(); page.setDefaultTimeout(15000);
    page.on('pageerror',e=>errors.push(e.name)); page.on('dialog',d=>d.accept()); await autoConfirm(page);
    await page.addInitScript(session=>{
      sessionStorage.setItem('nabiman_token',session.token); sessionStorage.setItem('nabiman_refresh',session.refresh_token);
      localStorage.setItem('nabiman_lang','en');
    },first);
    await page.goto(base+'/#administration/loginSessions');
    await page.getByRole('heading',{name:'Login sessions',exact:true}).waitFor();
    await page.getByText('Current login',{exact:true}).waitFor();
    const rows=page.locator('tbody tr');
    if (await rows.count()!==2 || (await rows.allTextContents()).some(t=>t.includes('admin'))) throw new Error('Cross-account session disclosure');
    await page.getByRole('button',{name:'Sign out my other logins',exact:true}).click();
    await page.getByText('Login sessions revoked.',{exact:true}).waitFor();
    await page.waitForFunction(()=>document.querySelectorAll('tbody tr').length===1);
    if ((await request('/api/users/me',second.token,undefined,'GET')).status!==401) throw new Error('Revoked access token accepted');
    if ((await request('/api/auth/refresh',null,{refresh_token:second.refresh_token})).status!==401) throw new Error('Revoked refresh accepted');
    if ((await request('/api/users/me',first.token,undefined,'GET')).status!==200) throw new Error('Current login revoked by others action');
    const all=await request('/api/auth/sessions',admin.token,undefined,'GET');
    const root=all.body.data.find(s=>s.username==='admin');
    if ((await request('/api/auth/sessions/revoke',first.token,{session_id:root.session_id})).status!==404) throw new Error('Viewer revoked another account');
    const oldId=oldUser.id;
    if (!(await request('/api/users/'+oldId,admin.token,undefined,'DELETE')).body.success) throw new Error('Fixture account deletion failed');
    const replacement=await create();
    if (replacement.id===oldId) throw new Error('Account identity reused');
    if ((await request('/api/users/me',first.token,undefined,'GET')).status!==401) throw new Error('Old login adopted replacement account');
    if ((await request('/api/auth/refresh',null,{refresh_token:first.refresh_token})).status!==401) throw new Error('Old refresh adopted replacement account');
    const fresh=await login('sessionviewer');
    if ((await request('/api/users/me',fresh.token,undefined,'GET')).status!==200) throw new Error('Fresh account cannot log in');
    if (errors.length) throw new Error('Browser errors');
    console.log('PASS: actual browser self-session privacy and revoke-others; revoked access/refresh rejected, current login survives; cross-account revocation denied; delete/recreate does not adopt old login.');
  } finally { await browser.close(); }
})().catch(e=>{console.error(e.message);process.exit(1);});
