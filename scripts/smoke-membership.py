#!/usr/bin/env python3
"""Exercise signup, approval, delegation, resource isolation and revocation on an isolated server."""
import json, os, pathlib, socket, sqlite3, subprocess, tempfile, time
from urllib.request import Request, urlopen
from urllib.error import HTTPError

ROOT = pathlib.Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='nabiman-membership-') as directory:
    data = pathlib.Path(directory)
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0)); port = sock.getsockname()[1]
    env = os.environ.copy()
    env.update(NABIMAN_PORT=str(port), NABIMAN_DATA_DIR=str(data), NABIMAN_MEC_DB=str(data/'mec.db'),
               NABIMAN_MEC_MODE='mock', NABIMAN_STATIC=str(ROOT/'frontend/build'), NABIMAN_PASSWORD='isolated-admin-password-123')
    log = (data/'server.log').open('w')
    server = subprocess.Popen([os.environ.get('NABIMAN_SMOKE_BINARY', str(ROOT/'backend/target/debug/nabiman-server'))], env=env, cwd=data, stdout=log, stderr=log)
    def request(path, body=None, token=None, method=None, extra_headers=None):
        headers = {'Content-Type':'application/json'}
        if token: headers['Authorization']='Bearer '+token
        if extra_headers: headers.update(extra_headers)
        req = Request(f'http://127.0.0.1:{port}'+path, None if body is None else json.dumps(body).encode(), headers,
                      method=method or ('GET' if body is None else 'POST'))
        try:
            with urlopen(req,timeout=10) as r: return r.status, json.load(r)
        except HTTPError as e:
            return e.code, json.loads(e.read())
    def save_policy(path,body,token):
        _,current=request(path,token=token)
        return request(path,{**body,'expected_revision':(current.get('data') or {}).get('revision','not-visible'),'change_reason':'Scoped membership policy verification'},token,method='PUT')
    def login(name, password='isolated-member-password-123'):
        code, res = request('/api/auth/login', {'username':name,'password':password})
        assert code == 200 and res['success'], (code,res)
        return res['data']['token']
    def m(kind, id, role): return {'kind':kind,'scope_id':id,'role':role}
    try:
        for _ in range(100):
            try:
                admin = login('admin','isolated-admin-password-123'); break
            except (OSError, AssertionError):
                if server.poll() is not None: raise AssertionError('Server startup failed')
                time.sleep(.1)
        else: raise AssertionError('Server startup timeout')
        servers = []
        for name in ['one','two']:
            code, res = request('/api/remote-servers', {'name':name,'host':'127.0.0.1','port':1,'user':'test'},admin)
            assert res['success'],res
            servers.append(res['data']['id'])
        with sqlite3.connect(data/'mec.db') as db:
            for name in ['tenant-one','tenant-two']:
                db.execute('INSERT INTO tenant_profiles (id,display_name,namespace,allocation_json,quota_json,status,created_at,created_by,updated_at) VALUES (?,?,?,?,?,?,?,?,?)',
                    (name,name,name,json.dumps({'type':'shared'}),json.dumps({'cpu_requests':'1','cpu_limits':'2','memory_requests':'1Gi','memory_limits':'2Gi','gpu':0,'storage':'10Gi','pods':10,'pvcs':2,'lb_services':1}),json.dumps({'state':'active'}),'2026-10-04T00:00:00Z','admin','2026-10-04T00:00:00Z'))
        for name in ['manager','member','outsider']:
            code,res=request('/api/auth/register',{'username':name,'password':'isolated-member-password-123','role':'admin'})
            assert code==201 and res['success'],res
            code,res=request('/api/auth/login',{'username':name,'password':'isolated-member-password-123'})
            assert code==403 and 'token' not in (res.get('data') or {}),res
        _,res=request('/api/users',token=admin); ids={u['username']:u['id'] for u in res['data']}
        for name, index, role in [('manager',0,'admin'),('member',0,'viewer'),('outsider',1,'viewer')]:
            scopes=[m('server',servers[index],role),m('tenant','tenant-one' if index==0 else 'tenant-two',role)]
            code,res=request(f'/api/users/{ids[name]}/review',{'status':'active','memberships':scopes},admin)
            assert code==200 and res['success'],res
        manager=login('manager'); member=login('member')
        # Authentication, not caller-supplied labels, defines the audit actor.
        audit_probe=f'/api/remote-servers/{servers[0]}/check'
        assert request(audit_probe,{},member,extra_headers={'X-User':'forged-admin'})[0]==200
        assert request('/api/users',token='invalid-token',extra_headers={'X-User':'admin'})[0]==401
        _,audit=request('/api/audit?path_filter='+audit_probe,token=admin)
        assert audit['data'][0]['user']=='member',audit
        _,audit=request('/api/audit?path_filter=/api/users',token=admin)
        assert any(e['status']==401 and e['user']=='unauthenticated' for e in audit['data']),audit
        print('PASS: verified audit identity ignores spoofed actor headers and records rejected authentication')
        _,res=request('/api/users',token=manager)
        assert {u['username'] for u in res['data']}=={'manager','member'},res
        assert all('password_hash' not in u and 'totp_secret' not in u for u in res['data'])
        _,res=request('/api/remote-servers',token=manager); assert [s['id'] for s in res['data']]==servers[:1],res
        _,res=request('/api/mec/v1/tenants',token=manager); assert [t['id'] for t in res['data']]==['tenant-one'],res
        for path in ['/api/mec/v1/tenants/tenant-two','/api/mec/v1/nodes','/api/files/download','/api/terminal','/api/auth/oauth/config']:
            assert request(path,token=manager)[0]==403,path
        for path in [f'/api/remote-servers/{servers[1]}/check',f'/api/users/{ids["member"]}/review','/api/users']:
            assert request(path,{},manager)[0]==403,path
        assert request('/api/users/memberships',{'username':'member','membership':m('server',servers[0],'admin')},manager)[0]==403
        assert request('/api/users/memberships',{'username':'outsider','membership':m('server',servers[1],'viewer')},manager)[0]==403
        code,res=request('/api/users/memberships',{'username':'member','membership':m('server',servers[0],'operator')},manager)
        assert code==200 and res['success'],res
        assert request('/api/users/me',token=member)[0]==401
        member=login('member')
        _,res=request('/api/users/me',token=member)
        assert res['data']['memberships'][1]['role']=='operator',res
        # Host identity is managed only by the matching scope admin or highest admin.
        key_path = data/'host-key'
        subprocess.run(['ssh-keygen','-q','-t','ed25519','-N','','-f',str(key_path)],check=True)
        public_key = pathlib.Path(str(key_path)+'.pub').read_text().strip()
        fingerprint = subprocess.check_output(['ssh-keygen','-l','-E','sha256','-f',str(key_path)+'.pub'],text=True).split()[1]
        trust_path = f'/api/remote-servers/{servers[0]}/host-trust'
        _, current_servers = request('/api/remote-servers',token=manager)
        initial_version = next(s['host_trust_version'] for s in current_servers['data'] if s['id']==servers[0])
        trust = {'host':'127.0.0.1','port':1,'public_key':public_key,'fingerprint':fingerprint,'previous_fingerprint':'','previous_version':initial_version,'reason':'Verified via isolated console'}
        assert request(trust_path,trust,member,method='PUT')[0]==403
        assert request(f'/api/remote-servers/{servers[1]}/host-trust',trust,manager,method='PUT')[0]==403
        assert request(trust_path,{**trust,'fingerprint':'SHA256:wrong'},manager,method='PUT')[0]==400
        code,res=request(trust_path,trust,manager,method='PUT')
        assert code==200 and res['success'] and res['data']['host_trust']['verified_by']=='manager',res
        assert not res['data']['password_enc']
        assert request(trust_path,trust,manager,method='PUT')[0]==409
        assert request(trust_path,{**trust,'host':'wrong.example','previous_fingerprint':fingerprint},manager,method='PUT')[0]==409
        _,listed=request('/api/remote-servers',token=member)
        assert listed['data'][0]['host_trust']['fingerprint']==fingerprint
        first_version=res['data']['host_trust_version']
        replacement={**trust,'previous_fingerprint':fingerprint,'previous_version':first_version}
        code,res=request(trust_path,replacement,manager,method='PUT')
        assert code==200 and res['success']
        second_version=res['data']['host_trust_version']
        assert second_version != first_version
        stale_revoke={**replacement,'public_key':'','fingerprint':''}
        assert request(trust_path,stale_revoke,manager,method='PUT')[0]==409
        code,res=request(trust_path,{**stale_revoke,'previous_version':second_version},manager,method='PUT')
        assert code==200 and res['success']
        revoked_version=res['data']['host_trust_version']
        assert revoked_version not in [initial_version,first_version,second_version]
        assert request(trust_path,trust,admin,method='PUT')[0]==409  # empty-key ABA
        assert request(trust_path,{**trust,'previous_version':revoked_version},admin,method='PUT')[1]['success']
        # Moving a saved endpoint must require independent trust again.
        assert request(f'/api/remote-servers/{servers[0]}',{'port':2},admin,method='PUT')[1]['data']['host_trust'] is None
        assert request(f'/api/remote-servers/{servers[0]}',{'port':1},admin,method='PUT')[1]['success']
        subprocess.run(['node',str(ROOT/'frontend/scripts/smoke-host-trust-browser.js'),f'http://127.0.0.1:{port}'],input=json.dumps({'public_key':public_key,'fingerprint':fingerprint}),text=True,check=True)
        print('PASS: host trust role boundaries, fingerprint verification, same-key approval generations, stale revoke/empty-trust ABA denial and target change invalidation')
        # Persisted scope policies are writable only by the matching manager.
        policy_path=f'/api/scope-settings/server/{servers[0]}'
        policy={'contact_email':'manager@example.test','member_notice':'Own scope notice','member_limit':2,'allow_commands':False}
        assert save_policy(policy_path,policy,member)[0]==403
        assert save_policy(f'/api/scope-settings/server/{servers[1]}',policy,manager)[0]==403
        assert save_policy(policy_path,policy,manager)[1]['success']
        stored_policy=request(policy_path,token=member)[1]['data']; assert stored_policy.pop('revision')
        assert stored_policy=={**policy,'require_mfa':False}
        assert request(f'/api/remote-servers/{servers[0]}/exec',{'command':'true'},member)[0]==403
        assert not request('/api/users/memberships',{'username':'outsider','membership':m('server',servers[0],'viewer')},manager)[1]['success']
        assert not save_policy(policy_path,{**policy,'member_limit':1},manager)[1]['success']
        assert (data/'scope-settings.json').stat().st_mode & 0o777 == 0o600
        # Per-member limits and expiry are checked even with a newly issued token.
        for access in [{'allow_exec':False}, {'enabled':False}, {'expires_at':'2000-01-01T00:00:00Z'}]:
            grant={**m('server',servers[0],'operator'),**access}
            assert request('/api/users/memberships',{'username':'member','membership':grant},manager)[1]['success']
            assert request('/api/users/me',token=member)[0]==401
            member=login('member')
            assert request(f'/api/remote-servers/{servers[0]}/exec',{'command':'true'},member)[0]==403
            if 'enabled' in access or 'expires_at' in access:
                assert request(f'/api/remote-servers/{servers[0]}/check',{},member)[0]==403
                _,visible_servers=request('/api/remote-servers',token=member)
                assert not visible_servers['data']
        assert not request('/api/users/memberships',{'username':'member','membership':{**m('server',servers[0],'operator'),'expires_at':'bad'}},manager)[1]['success']
        assert request('/api/users/memberships',{'username':'member','membership':m('server',servers[0],'operator')},manager)[1]['success']
        policy['allow_commands']=True; policy['member_limit']=None
        assert save_policy(policy_path,policy,manager)[1]['success']
        print('PASS: scope policy persistence, cross-tenant settings rejection, member limit, command restriction, disabled and expired access')
        code,res=request(f'/api/users/{ids["manager"]}/review',{'status':'suspended','memberships':[]},admin)
        assert code==200 and res['success'],res
        assert request('/api/users/me',token=manager)[0]==401
        stored=json.loads((data/'users.json').read_text())
        assert all(u['password_hash'] for u in stored)
        assert next(u for u in stored if u['username']=='member')['memberships']
        assert next(u for u in stored if u['username']=='manager')['status']=='suspended'
        # Last super administrator cannot be suspended or demoted.
        assert not request(f'/api/users/{ids["admin"]}/review',{'status':'suspended','memberships':[]},admin)[1]['success']
        assert request(f'/api/users/{ids["admin"]}',{'role':'viewer'},admin,method='PUT')[0]==400
        subprocess.run(['node',str(ROOT/'frontend/scripts/smoke-membership-browser.js'),f'http://127.0.0.1:{port}'],check=True)
        print('PASS: signup/pending login, approval, server + tenant isolation, delegated membership management, privilege escalation rejection, session revocation, persistence, last-admin protection')
    except Exception:
        log.flush()
        print((data/'server.log').read_text()[-2000:])
        raise
    finally:
        server.terminate()
        try: server.wait(timeout=5)
        except subprocess.TimeoutExpired: server.kill(); server.wait()
        log.close()
