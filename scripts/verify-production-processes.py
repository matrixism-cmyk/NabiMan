#!/usr/bin/env python3
"""Read-only production process UI verification; sends no termination/stop signals."""
import argparse,importlib.util,json,os,pathlib,subprocess
spec=importlib.util.spec_from_file_location('d',pathlib.Path(__file__).with_name('deploy-reviewed-runtime.py'));d=importlib.util.module_from_spec(spec);spec.loader.exec_module(d)
p=argparse.ArgumentParser();p.add_argument('--deployment',type=pathlib.Path,required=True);args=p.parse_args()
assert os.geteuid()==0 and args.deployment.parent==d.ROOT/'deployments';os.umask(0o077)
session=d.login(d.PUBLIC)
try:
 code,value=d.api(d.PUBLIC,'/api/processes',session['token']);assert code==200 and value['success'] and value['data'];rows=value['data'];assert all(len(r['identity'])==64 and r['pid']>0 for r in rows)
 code,value=d.api(d.PUBLIC,'/api/processes/options',session['token']);assert code==200 and value['data']['can_signal']
 result=subprocess.run(['node',str(d.SOURCE/'frontend/scripts/verify-production-processes.js')],input=json.dumps({'base':d.PUBLIC,'token':session['token'],'chromiumPath':d.CHROME,'screenshot':str(args.deployment/'evidence/processes.png')}),capture_output=True,text=True,timeout=60,cwd=d.SOURCE/'frontend')
 assert result.returncode==0,'Read-only process browser verification failed'
 proof={'passed':True,'browser':json.loads(result.stdout),'kernel_pidfd_capability':True,'sampled_processes':len(rows),'runtime':d.check(d.PUBLIC,d.read(args.deployment/'plan.json'))}
 d.write(args.deployment/'process-verification.json',proof);print(json.dumps(proof))
finally:d.api(d.PUBLIC,'/api/auth/logout',session['token'],{})
