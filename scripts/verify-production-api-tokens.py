#!/usr/bin/env python3
"""Issue/use/revoke one scoped production probe through the real browser, retaining no secret."""
import argparse,importlib.util,json,os,pathlib,subprocess,uuid
spec=importlib.util.spec_from_file_location('d',pathlib.Path(__file__).with_name('deploy-reviewed-runtime.py'));d=importlib.util.module_from_spec(spec);spec.loader.exec_module(d)
parser=argparse.ArgumentParser();parser.add_argument('--deployment',type=pathlib.Path,required=True);args=parser.parse_args()
assert os.geteuid()==0 and args.deployment.parent==d.ROOT/'deployments';os.umask(0o077)
plan=d.read(args.deployment/'plan.json');d.verify_artifacts(args.deployment,plan);assert d.read(args.deployment/'activated.json')['deployed']
session=d.login(d.PUBLIC);token=session['token'];name='deployment-token-probe-'+uuid.uuid4().hex;ids=set()
try:
 code,value=d.api(d.PUBLIC,'/api/apikeys',token);assert code==200;before=value['data'];ids={k['id'] for k in before}
 payload={'base':d.PUBLIC,'token':token,'name':name,'chromiumPath':d.CHROME,'screenshot':str(args.deployment/'tokens-production.png')}
 result=subprocess.run(['node',str(d.SOURCE/'frontend/scripts/smoke-api-tokens.js')],cwd=d.SOURCE/'frontend',input=json.dumps(payload),text=True,capture_output=True,timeout=90)
 assert result.returncode==0,'Public token browser proof failed; no credential-bearing browser log is printed'
 proof=json.loads(result.stdout)
 code,value=d.api(d.PUBLIC,'/api/apikeys',token);assert code==200 and value['data']==before,'Token inventory changed after probe'
 proof.update(passed=True,original_inventory_preserved=True,retained_probe_credentials=0)
 d.write(args.deployment/'tokens-verification.json',proof);print(json.dumps(proof))
finally:
 code,value=d.api(d.PUBLIC,'/api/apikeys',token)
 if code==200:
  for row in value['data']:
   if row['name']==name and row['id'] not in ids:assert d.api(d.PUBLIC,'/api/apikeys/'+row['id'],token,method='DELETE')[0]==200
 d.api(d.PUBLIC,'/api/auth/logout',token,{})
