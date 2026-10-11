#!/usr/bin/env python3
"""Verify a nonexistent account's limit without changing or locking any production account."""
import argparse,importlib.util,json,os,pathlib,subprocess,uuid
spec=importlib.util.spec_from_file_location('d',pathlib.Path(__file__).with_name('deploy-reviewed-runtime.py'));d=importlib.util.module_from_spec(spec);spec.loader.exec_module(d)
parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--deployment',type=pathlib.Path,required=True);args=parser.parse_args()
assert os.geteuid()==0 and args.deployment.parent==d.ROOT/'deployments';os.umask(0o077)
plan=d.read(args.deployment/'plan.json');assert d.read(args.deployment/'activated.json')['deployed'];d.verify_artifacts(args.deployment,plan)
blocked='admission-probe-'+uuid.uuid4().hex[:12]
assert all(u['username']!=blocked for u in d.read(d.AUTH/'users.json'))
for _ in range(5):assert d.api(d.PUBLIC,'/api/auth/login',body={'username':blocked,'password':'nonexistent-fixture-password'})[0]==401
request=d.Request(d.PUBLIC+'/api/auth/login',data=json.dumps({'username':blocked,'password':'nonexistent-fixture-password'}).encode(),headers={'Content-Type':'application/json','X-Forwarded-For':'203.0.113.123'},method='POST')
try:response=d.urlopen(request,timeout=20)
except d.HTTPError as error:response=error
with response:
 assert response.status==429 and 1<=int(response.headers['Retry-After'])<=300
 assert response.headers['Cache-Control']=='no-store'
payload={'base':d.PUBLIC,'blocked':blocked,'chromiumPath':d.CHROME,'password':(d.ROOT/'secrets/admin_password').read_text().rstrip('\r\n')}
result=subprocess.run(['node',str(d.SOURCE/'frontend/scripts/smoke-auth-admission.js')],input=json.dumps(payload),text=True,capture_output=True,timeout=60,cwd=d.SOURCE/'frontend')
assert result.returncode==0,'Public authentication browser proof failed; credential-bearing output is not printed'
assert all(u['username']!=blocked for u in d.read(d.AUTH/'users.json'))
proof={'passed':True,'unknown_account_limit_verified':True,'retry_after_and_no_store':True,'no_production_account_failed_passwords':True,'no_probe_account_created':True,'browser':json.loads(result.stdout),'runtime':d.check(d.PUBLIC,plan),'limitations':['Only a unique nonexistent-account limit was exercised in production','Global flood and slow-body tests were isolated fixtures']}
d.write(args.deployment/'auth-admission-verification.json',proof);print(json.dumps(proof))
