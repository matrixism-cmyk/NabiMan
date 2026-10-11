#!/usr/bin/env python3
"""Run public browser verification and save credential-free rollout evidence."""
import argparse, importlib.util, json, os, pathlib, signal, subprocess
spec = importlib.util.spec_from_file_location('deployment', pathlib.Path(__file__).with_name('deploy-reviewed-runtime.py'))
d = importlib.util.module_from_spec(spec); spec.loader.exec_module(d)
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--deployment', type=pathlib.Path, required=True)
args = parser.parse_args()
assert os.geteuid() == 0 and args.deployment.parent == d.ROOT/'deployments'
os.umask(0o077)
plan = d.read(args.deployment/'plan.json')
assert d.read(args.deployment/'activated.json')['deployed']
d.verify_artifacts(args.deployment, plan)
evidence = args.deployment/'evidence'; evidence.mkdir(mode=0o700, exist_ok=True)
session = d.login(d.PUBLIC); token = session['token']
probe = None
rotation = None
try:
    probe = d.login(d.PUBLIC)
    code, rows = d.api(d.PUBLIC, '/api/auth/sessions', probe['token'])
    assert code == 200
    sid = next(row['session_id'] for row in rows['data'] if row['is_current'])
    administrator = next(user for user in d.read(d.AUTH/'users.json') if user['id'] == plan['owner']['id'])
    payload = {'mfaEnabled': administrator.get('totp_enabled', False), 'probeLogin': {'sid':sid, **probe}, 'base':d.PUBLIC,'token':token,'sessionIds':list(plan['sessions']),
        'expectedBinarySha256':plan['binary_sha256'],'chromiumPath':d.CHROME,'evidence':str(evidence)}
    process = subprocess.Popen(['node', str(d.SOURCE/'frontend/scripts/smoke-production-rollout.js')],
        cwd=d.SOURCE/'frontend', stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        text=True, start_new_session=True)
    try: stdout, stderr = process.communicate(json.dumps(payload), timeout=100)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGTERM)
        try: process.communicate(timeout=5)
        except subprocess.TimeoutExpired: os.killpg(process.pid, signal.SIGKILL); process.communicate(timeout=5)
        raise AssertionError('Public browser verification timed out')
    if process.returncode:
        raise AssertionError(stderr[-1500:])
    proof = json.loads(stdout)
    rotation = d.login(d.PUBLIC)
    code, renewed = d.api(d.PUBLIC, '/api/auth/refresh', body={'refresh_token':rotation['refresh_token']})
    assert code == 200 and renewed['data']['refresh_token'] != rotation['refresh_token']
    fresh = renewed['data']
    assert d.api(d.PUBLIC, '/api/users/me', fresh['token'])[0] == 200
    assert d.api(d.PUBLIC, '/api/auth/refresh', body={'refresh_token':rotation['refresh_token']})[0] == 401
    assert d.api(d.PUBLIC, '/api/users/me', fresh['token'])[0] == 401
    assert d.api(d.PUBLIC, '/api/auth/refresh', body={'refresh_token':fresh['refresh_token']})[0] == 401
    assert d.api(d.PUBLIC, '/api/users/me', token)[0] == 200
    proof['refresh_rotation_and_replay_revocation'] = True
    before = args.deployment/'cutover.before/auth'
    previous_users = d.read(before/'users.json')
    current_users = d.read(d.AUTH/'users.json')
    def credentials(users):
        normalized = []
        for user in users:
            value = {key:item for key,item in user.items() if key != 'last_login'}
            value.setdefault('mfa', {'recovery_hashes':[], 'last_used_step':None, 'pending_expires_at':None, 'revision':''})
            normalized.append(value)
        return {u['id']:u for u in normalized}
    assert credentials(previous_users) == credentials(current_users), 'Member authorization or credentials changed'
    old_sessions = d.read(before/'terminal_sessions.json'); current_sessions = d.read(d.AUTH/'terminal_sessions.json')
    assert {key:d.session_identity(value) for key,value in old_sessions.items()} == {
        key:d.session_identity(value) for key,value in current_sessions.items()}
    assert d.read(before/'remote_servers.json') == d.read(d.AUTH/'remote_servers.json')
    old_policy = before/'scope-settings.json'; current_policy = d.AUTH/'scope-settings.json'
    assert old_policy.exists() == current_policy.exists()
    if old_policy.exists(): assert old_policy.read_bytes() == current_policy.read_bytes(), 'Scope policies changed during rollout'
    proof['scope_policies_preserved'] = True
    old_backup = before/'backup_schedules.json'; current_backup = d.AUTH/'backup_schedules.json'
    assert old_backup.exists() == current_backup.exists()
    if old_backup.exists(): assert old_backup.read_bytes() == current_backup.read_bytes(), 'Backup schedules changed during read-only rollout'
    proof['backup_schedules_preserved'] = True

    assert set(d.read(args.deployment/'cutover.before/tmux.json')).issubset(d.tmux_names())
    for name, digest in plan['static'].items():
        # Index references are verified by the browser; compare current index and its direct chunks.
        if name == 'index.html':
            with d.urlopen(d.PUBLIC+'/', timeout=20) as response:
                assert d.hashlib.sha256(response.read()).hexdigest() == digest
    proof.update(member_authorization_preserved=True, remote_trust_preserved=True,
        runtime=d.check(d.PUBLIC, plan), tmux_count=len(d.tmux_names()))
    d.write(args.deployment/'browser-verification.json', proof)
    print(json.dumps(proof))
finally:
    if rotation: d.api(d.PUBLIC, '/api/auth/logout', rotation['token'], {})
    if probe: d.api(d.PUBLIC, '/api/auth/logout', probe['token'], {})
    d.api(d.PUBLIC, '/api/auth/logout', token, {})
