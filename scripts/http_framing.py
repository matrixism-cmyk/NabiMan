"""Bounded wire probes for an isolated release fixture, never the public proxy."""
import http.client
import json
import socket


def verify(port, password):
    cases = {
        'content_length_plus_chunked': b'Host: localhost\r\nContent-Length: 0\r\nTransfer-Encoding: chunked\r\n',
        'chunked_plus_content_length': b'Host: localhost\r\nTransfer-Encoding: chunked\r\nContent-Length: 6\r\n',
        'duplicate_content_length': b'Host: localhost\r\nContent-Length: 0\r\nContent-Length: 6\r\n',
        'duplicate_transfer_encoding': b'Host: localhost\r\nTransfer-Encoding: chunked\r\nTransfer-Encoding: chunked\r\n',
        'nonfinal_chunked': b'Host: localhost\r\nTransfer-Encoding: chunked, gzip\r\n',
        'identity_transfer_encoding': b'Host: localhost\r\nTransfer-Encoding: identity\r\n',
        'duplicate_host': b'Host: localhost\r\nHost: other.invalid\r\nContent-Length: 0\r\n',
        'missing_host': b'Content-Length: 0\r\n',
    }
    for name, headers in cases.items():
        with socket.create_connection(('127.0.0.1', port), timeout=5) as peer:
            peer.sendall(b'POST /api/auth/login HTTP/1.1\r\n'+headers+b'\r\n0\r\n\r\nGET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n')
            raw=b''
            while True:
                try: block=peer.recv(4096)
                except ConnectionResetError: break
                if not block: break
                raw+=block
                assert len(raw)<16384, 'Unexpected unbounded response: '+name
            assert raw.startswith(b'HTTP/1.1 400'), 'Ambiguous request was not rejected: '+name
            assert raw.count(b'HTTP/1.1 ')==1, 'Pipelined request processed after rejection: '+name
    # Legitimate streaming request body plus trailers must still reach the login handler.
    body=json.dumps({'username':'admin','password':password}).encode()
    with socket.create_connection(('127.0.0.1', port), timeout=5) as peer:
        peer.sendall(b'POST /api/auth/login HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nTrailer: X-Fixture\r\nConnection: close\r\n\r\n'+format(len(body),'x').encode()+b'\r\n'+body+b'\r\n0\r\nX-Fixture: complete\r\n\r\n')
        response=http.client.HTTPResponse(peer);response.begin()
        assert response.status==200, 'Valid chunked request rejected'
        result=json.loads(response.read(32768));assert result['success'] and result['data']['token']
    return {'ambiguous_requests_rejected':list(cases),'rejected_connections_closed':True,'pipelined_request_after_error_not_processed':True,'valid_chunked_body_and_trailer':True,'scope':'isolated backend TCP; not a proxy or load test'}
