#!/usr/bin/env python3
"""Verify retained original bytes and new v2 records after the authorized audit rollout."""
import argparse,hashlib,importlib.util,json,os,pathlib
spec=importlib.util.spec_from_file_location('deployment',pathlib.Path(__file__).with_name('deploy-reviewed-runtime.py'));d=importlib.util.module_from_spec(spec);spec.loader.exec_module(d)
parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--deployment',type=pathlib.Path,required=True);args=parser.parse_args()
assert os.geteuid()==0 and args.deployment.parent==d.ROOT/'deployments';os.umask(0o077)
assert d.read(args.deployment/'activated.json')['deployed']
plan=d.read(args.deployment/'plan.json');d.verify_artifacts(args.deployment,plan)
names=['audit.jsonl']+[f'audit.jsonl.{i}' for i in range(1,6)]
before=args.deployment/'cutover.before/auth'
current={name:(d.AUTH/name).read_bytes() for name in names if (d.AUTH/name).exists()}
preserved=[]
for name in names:
 if not (before/name).exists():continue
 source=(before/name).read_bytes();matches=[candidate for candidate,content in current.items() if content.startswith(source)]
 assert matches,'Original audit bytes were changed or lost: '+name
 preserved.append({'original':name,'retained_in':matches,'bytes':len(source),'sha256':hashlib.sha256(source).hexdigest()})
verified=0
for content in current.values():
 previous='0'
 for line in content.splitlines():
  try:entry=json.loads(line)
  except ValueError:continue
  actual=entry.get('hash')
  if isinstance(actual,str) and actual.startswith('v2:'):
   values=[previous,entry['timestamp'],entry['user'],entry['method'],entry['path'],entry['status'],entry['ip']]
   expected='v2:'+hashlib.sha256(b'nabiman-audit-v2\0'+json.dumps(values,ensure_ascii=False,separators=(',',':')).encode()).hexdigest()
   assert actual==expected,'New audit record hash mismatch'
   verified+=1
  if actual:previous=actual
assert verified>0,'No newly persisted versioned audit records'
session=d.login(d.PUBLIC)
try:
 code,health=d.api(d.PUBLIC,'/api/audit/health',session['token']);assert code==200 and health['data']['writable']
 code,inspection=d.api(d.PUBLIC,'/api/audit/verify',session['token']);assert code==200 and inspection['success']
 assert inspection['data']['complete_history_proven'] is False and inspection['data']['independent_proof'] is False
 proof={'passed':True,'old_file_bytes_preserved':preserved,'new_versioned_records_independently_verified':verified,'writer':health['data'],'inspection':inspection['data'],'limits':['Local retained-file inspection is not independent immutable audit','Historical malformed and mismatching records are preserved, not repaired']}
 d.write(args.deployment/'audit-verification.json',proof)
 print(json.dumps({'passed':True,'preserved_files':len(preserved),'new_versioned_records_verified':verified,'writer_available':True,'historical_integrity_valid':inspection['data']['valid']}))
finally:d.api(d.PUBLIC,'/api/auth/logout',session['token'],{})
