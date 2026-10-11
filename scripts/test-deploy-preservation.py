#!/usr/bin/env python3
"""Ensure repeat deployment neither rotates trust nor adopts changed terminal owners."""
import copy, importlib.util, json, pathlib, tempfile, unittest
spec = importlib.util.spec_from_file_location('deployment', pathlib.Path(__file__).with_name('deploy-reviewed-runtime.py'))
deployment = importlib.util.module_from_spec(spec); spec.loader.exec_module(deployment)

class Preservation(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = pathlib.Path(self.temp.name)
        self.owner = {'id':'immutable-admin','username':'admin','created_at':'original'}
        self.sessions = {'s1':{'owner':'admin','owner_id':'immutable-admin','tmux_name':'existing',
            'label':'remote','ssh_cmd':'existing command','remote_binding':{'approval_version':'approved-version'}}}
        self.servers = [{'id':'remote','host':'example.test','port':22,'user':'operator',
            'auth_method':'key','host_trust':{'public_key':'approved-key'},'host_trust_version':'approved-version'}]
        self.plan = {'mode':'preserve','owner':self.owner,
            'sessions':{'s1':{'identity':deployment.session_identity(self.sessions['s1'])}},
            'servers':{'remote':{'identity':deployment.server_identity(self.servers[0])}}}
        self.put('users.json', [{**self.owner,'role':'admin','status':'active','password_hash':'unchanged'}])
        self.put('terminal_sessions.json', self.sessions); self.put('remote_servers.json', self.servers)
    def put(self, name, value): (self.root/name).write_text(json.dumps(value))
    def test_preserves_exact_files(self):
        before = {p.name:p.read_bytes() for p in self.root.iterdir()}
        deployment.migrate(self.root, self.plan)
        self.assertEqual(before, {p.name:p.read_bytes() for p in self.root.iterdir()})
    def test_changed_identity_rejected_without_writes(self):
        for key, value in [('owner_id',None),('owner_id','different'),('remote_binding',{'approval_version':'new'})]:
            with self.subTest(key=key,value=value):
                changed = copy.deepcopy(self.sessions); changed['s1'][key] = value
                self.put('terminal_sessions.json', changed)
                with self.assertRaises(AssertionError): deployment.migrate(self.root, self.plan)
                self.assertEqual(json.loads((self.root/'terminal_sessions.json').read_text()), changed)
    def test_changed_trust_or_target_rejected(self):
        for key, value in [('host','other.test'),('host_trust',None),('host_trust_version','new')]:
            with self.subTest(key=key):
                changed = copy.deepcopy(self.servers); changed[0][key] = value
                self.put('remote_servers.json', changed)
                with self.assertRaises(AssertionError): deployment.migrate(self.root, self.plan)
                self.assertEqual(json.loads((self.root/'remote_servers.json').read_text()), changed)

if __name__ == '__main__': unittest.main()
