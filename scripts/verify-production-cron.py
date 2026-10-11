#!/usr/bin/env python3
"""Read-only production cron verification; never install or delete a scheduled command."""
import argparse,hashlib,importlib.util,json,os,pathlib,subprocess
spec=importlib.util.spec_from_file_location('d',pathlib.Path(__file__).with_name('deploy-reviewed-runtime.py'));d=importlib.util.module_from_spec(spec);spec.loader.exec_module(d)
p=argparse.ArgumentParser();p.add_argument('--deployment',type=pathlib.Path,required=True);args=p.parse_args()
assert os.geteuid()==0 and args.deployment.parent==d.ROOT/'deployments';os.umask(0o077)
def spool():return {str(p):hashlib.sha256(p.read_bytes()).hexdigest() for folder in ['/var/spool/cron/crontabs','/var/spool/cron'] if pathlib.Path(folder).exists() for p in pathlib.Path(folder).iterdir() if p.is_file()}
before=spool();plan=d.read(args.deployment/'plan.json');assert before==plan['cron_spool_digests'],'Cron spool changed since preflight'
session=d.login(d.PUBLIC)
try:
 code,value=d.api(d.PUBLIC,'/api/cron',session['token']);assert code==200 and value['success'];rows=value['data'];assert all(len(r['revision'])==64 and r['id']>0 for r in rows)
 code,value=d.api(d.PUBLIC,'/api/cron/snapshot?user=root',session['token']);assert code==200 and value['data']['can_write']
 result=subprocess.run(['node',str(d.SOURCE/'frontend/scripts/verify-production-cron.js')],input=json.dumps({'base':d.PUBLIC,'token':session['token'],'chromiumPath':d.CHROME,'screenshot':str(args.deployment/'evidence/cron.png')}),capture_output=True,text=True,timeout=60,cwd=d.SOURCE/'frontend')
 assert result.returncode==0,'Read-only cron browser verification failed'
 assert spool()==before
 proof={'passed':True,'browser':json.loads(result.stdout),'cron_spool_preserved_since_preflight':True,'listed_jobs':len(rows),'runtime':d.check(d.PUBLIC,plan)}
 d.write(args.deployment/'cron-verification.json',proof);print(json.dumps(proof))
finally:
 d.api(d.PUBLIC,'/api/auth/logout',session['token'],{})
 assert spool()==before,'Cron spool changed during verification'
