#!/usr/bin/env python3
"""Read-only OS account checks after deployment; never dispatch an OS mutation."""
import argparse,hashlib,importlib.util,json,os,pathlib,subprocess
spec=importlib.util.spec_from_file_location('d',pathlib.Path(__file__).with_name('deploy-reviewed-runtime.py'));d=importlib.util.module_from_spec(spec);spec.loader.exec_module(d)
p=argparse.ArgumentParser();p.add_argument('--deployment',type=pathlib.Path,required=True);args=p.parse_args()
assert os.geteuid()==0 and args.deployment.parent==d.ROOT/'deployments'
os.umask(0o077)
files=[pathlib.Path('/etc')/n for n in ['passwd','shadow','group','gshadow']]
files += [pathlib.Path('/usr/sbin')/n for n in ['useradd','userdel','chpasswd']]
before={p:hashlib.sha256(p.read_bytes()).hexdigest() for p in files}
session=d.login(d.PUBLIC)
try:
 code,value=d.api(d.PUBLIC,'/api/accounts',session['token']);assert code==200 and value['success']
 rows=value['data'];root=next(r for r in rows if r['username']=='root');assert root['uid']==0 and not root['can_manage']
 assert all(r['is_logged_in'] is None for r in rows)
 code,options=d.api(d.PUBLIC,'/api/accounts/options',session['token']);assert code==200 and options['data']['can_write']
 result=subprocess.run(['node',str(d.SOURCE/'frontend/scripts/verify-production-os-accounts.js')],input=json.dumps({'base':d.PUBLIC,'token':session['token'],'chromiumPath':d.CHROME,'screenshot':str(args.deployment/'evidence/os-accounts.png')}),capture_output=True,text=True,timeout=60,cwd=d.SOURCE/'frontend')
 assert result.returncode==0,'Read-only browser verification failed'
 proof={'passed':True,'browser':json.loads(result.stdout),'unknown_login_status_honest':True,'account_files_and_tools_unchanged':all(hashlib.sha256(p.read_bytes()).hexdigest()==before[p] for p in files),'runtime':d.check(d.PUBLIC,d.read(args.deployment/'plan.json'))}
 assert proof['account_files_and_tools_unchanged']
 d.write(args.deployment/'os-accounts-verification.json',proof);print(json.dumps(proof))
finally:
 d.api(d.PUBLIC,'/api/auth/logout',session['token'],{})
 assert all(hashlib.sha256(p.read_bytes()).hexdigest()==before[p] for p in files),'OS account files or tools changed'
