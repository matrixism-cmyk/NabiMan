#!/usr/bin/env python3
"""Standard isolated runtime rehearsal plus read-only host crontab compatibility."""
import argparse,hashlib,importlib.util,json,os,pathlib
spec=importlib.util.spec_from_file_location('d',pathlib.Path(__file__).with_name('deploy-reviewed-runtime.py'));d=importlib.util.module_from_spec(spec);spec.loader.exec_module(d)
p=argparse.ArgumentParser();p.add_argument('--deployment',type=pathlib.Path,required=True);args=p.parse_args()
assert os.geteuid()==0 and args.deployment.parent==d.ROOT/'deployments';os.umask(0o077)
plan=d.read(args.deployment/'plan.json');original=d.check

def checked(base,reviewed):
 result=original(base,reviewed);session=d.login(base)
 try:
  code,value=d.api(base,'/api/cron',session['token']);assert code==200 and value['success'],'Candidate cannot read actual cron inventory'
  code,snapshot=d.api(base,'/api/cron/snapshot?user=root',session['token']);assert code==200 and snapshot['data']['can_write']
  d.write(args.deployment/'cron-rehearsal.json',{'passed':True,'listed_jobs':len(value['data']),'host_reads_only':True,'root_snapshot_available':True})
 finally:d.api(base,'/api/auth/logout',session['token'],{})
 return result

d.check=checked
d.rehearse(args.deployment,plan)
for name,digest in plan['cron_spool_digests'].items():assert d.sha(pathlib.Path(name))==digest
