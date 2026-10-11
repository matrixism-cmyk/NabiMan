#!/usr/bin/env python3
"""Prepare an offline users.json upgrade; never overwrite the live database."""
import argparse, json, os, pathlib
from urllib.request import Request, urlopen
import bcrypt

parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--input', required=True, type=pathlib.Path)
parser.add_argument('--output', required=True, type=pathlib.Path)
parser.add_argument('--bootstrap-password-file', type=pathlib.Path)
parser.add_argument('--verify-url', help='Current server base URL; required to restore a missing legacy admin hash')
args=parser.parse_args()
if args.input.resolve()==args.output.resolve(): parser.error('Output must be separate from the live database')
if args.output.exists(): parser.error('Output already exists; choose a new output path')
users=json.loads(args.input.read_text())
restored=0
for u in users:
    if not u.get('password_hash'):
        if u.get('username')!='admin' or u.get('role')!='admin' or not args.bootstrap_password_file or not args.verify_url:
            parser.error('Missing password hash requires verified legacy admin credentials; other accounts require individual recovery')
        pw=args.bootstrap_password_file.read_text().rstrip('\r\n')
        if not 1<=len(pw.encode())<=72 or pw in ['nabiman','changeme']: parser.error('A valid private legacy admin password is required')
        request=Request(args.verify_url.rstrip('/')+'/api/auth/login',json.dumps({'username':'admin','password':pw}).encode(),{'Content-Type':'application/json'})
        with urlopen(request,timeout=10) as r: login=json.load(r)
        if not login.get('success') or not login.get('data',{}).get('token'): parser.error('Live admin verification failed')
        u['password_hash']=bcrypt.hashpw(pw.encode(),bcrypt.gensalt(rounds=10)).decode()
        restored+=1
    u.setdefault('status','active')
    u.setdefault('memberships',[])
    u.setdefault('requested_memberships',[])
    u.setdefault('scope_restricted',False)
if not any(u['role']=='admin' and u['status']=='active' for u in users): parser.error('At least one active super administrator is required')
args.output.parent.mkdir(parents=True,exist_ok=True)
fd=os.open(args.output,os.O_WRONLY|os.O_CREAT|os.O_EXCL,0o600)
with os.fdopen(fd,'w') as file:
    json.dump(users,file,ensure_ascii=False,indent=2);file.write('\n');file.flush();os.fsync(file.fileno())
print(f'Prepared {len(users)} accounts; restored {restored} verified legacy admin hash. Live database unchanged.')
