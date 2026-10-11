"""Malformed-frame checks against an isolated, ticket-authenticated terminal."""
import base64,os,socket,struct


def verify(port, request):
    cases={'reserved_bit':(0xc1,b'ignored'), 'invalid_close_code':(0x88,struct.pack('!H',1005)),
           'invalid_close_utf8':(0x88,struct.pack('!H',1000)+b'\xff'), 'one_byte_close':(0x88,b'x')}
    for name,(first,payload) in cases.items():
        code,value=request('/api/terminal/tickets',{'query':'history=1'})
        assert code==200
        ticket=value['data']['ticket']
        with socket.create_connection(('127.0.0.1',port),timeout=5) as peer:
            key=base64.b64encode(os.urandom(16)).decode()
            peer.sendall((f'GET /api/terminal?history=1&ticket={ticket} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: {key}\r\n\r\n').encode())
            data=b''
            while b'\r\n\r\n' not in data:
                part=peer.recv(4096);assert part,'Upgrade closed';data+=part;assert len(data)<16384
            headers,data=data.split(b'\r\n\r\n',1);assert headers.startswith(b'HTTP/1.1 101'),'Fixture WebSocket upgrade failed'
            mask=os.urandom(4)
            peer.sendall(bytes([first,0x80|len(payload)])+mask+bytes(v^mask[i%4] for i,v in enumerate(payload)))
            # Application may close the transport without a close frame on protocol errors.
            received=len(data)
            while True:
                try: part=peer.recv(4096)
                except ConnectionResetError:break
                if not part:break
                data+=part;received+=len(part);assert received<131072,'Unexpected output after invalid frame'
            while data:
                assert len(data)>=2,'Truncated response frame'
                opcode=data[0]&15;length=data[1]&127;offset=2
                assert not data[1]&128,'Server frame must not be masked'
                if length==126:length=struct.unpack('!H',data[2:4])[0];offset=4
                elif length==127:length=struct.unpack('!Q',data[2:10])[0];offset=10
                assert len(data)>=offset+length,'Truncated response payload'
                body=data[offset:offset+length];data=data[offset+length:]
                if opcode==8:
                    assert len(body)>=2 and struct.unpack('!H',body[:2])[0] in [1002,1007], 'Malformed frame was acknowledged as a normal close: '+name
    return {'invalid_frames_close_connection':list(cases),'scope':'isolated ticket-authenticated terminal; finite protocol probes'}
