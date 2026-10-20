type Lang = 'ko' | 'en' | 'ja';
type T = Record<string, Record<Lang, string>>;

// How a saved server is reached: SSH/Telnet for the terminal, SFTP/FTPS/FTP
// for files, and the check that finds out which of them work.
const t: T = {
  'remote.proto.terminal': { ko: '터미널 접속', en: 'Terminal', ja: 'ターミナル接続' },
  'remote.proto.files': { ko: '파일 접속', en: 'Files', ja: 'ファイル接続' },
  'remote.proto.none': { ko: '없음', en: 'None', ja: 'なし' },
  'remote.proto.ftpsOption': { ko: 'FTPS (TLS 암호화)', en: 'FTPS (TLS)', ja: 'FTPS (TLS 暗号化)' },
  'remote.proto.ftpOption': { ko: 'FTP (암호화 안 됨)', en: 'FTP (unencrypted)', ja: 'FTP (暗号化なし)' },
  'remote.proto.telnetOption': { ko: 'Telnet (암호화 안 됨)', en: 'Telnet (unencrypted)', ja: 'Telnet (暗号化なし)' },
  'remote.proto.sshPort': { ko: 'SSH 포트', en: 'SSH port', ja: 'SSH ポート' },
  'remote.proto.telnetPort': { ko: 'Telnet 포트', en: 'Telnet port', ja: 'Telnet ポート' },
  'remote.proto.ftpPort': { ko: 'FTP 포트', en: 'FTP port', ja: 'FTP ポート' },
  'remote.proto.plainWarning': {
    ko: '{names}은(는) 비밀번호와 주고받는 내용을 암호화하지 않습니다. 믿을 수 있는 내부망에서만 쓰세요.',
    en: '{names} sends the password and everything else unencrypted. Use it only on a network you trust.',
    ja: '{names} はパスワードも内容も暗号化せずに送ります。信頼できる内部ネットワークでのみ使ってください。',
  },
  'remote.proto.fingerprint': { ko: 'FTPS 인증서 지문 (선택)', en: 'FTPS certificate fingerprint (optional)', ja: 'FTPS 証明書のフィンガープリント (任意)' },
  'remote.proto.fingerprintHint': {
    ko: '공인 인증서가 아니면(자체 서명 등) 서버 관리자에게 따로 확인한 SHA-256 지문을 넣으세요. 비워 두면 공인 인증서만 받아들입니다.',
    en: 'For a certificate no public authority signs (self-signed), paste the SHA-256 fingerprint you verified with the server\'s administrator. Left empty, only publicly trusted certificates are accepted.',
    ja: '公的な証明書でない場合 (自己署名など) は、サーバー管理者に別途確認した SHA-256 フィンガープリントを入力してください。空欄なら公的な証明書のみ受け付けます。',
  },
  'remote.proto.needOne': { ko: '터미널과 파일 중 하나는 골라야 합니다.', en: 'Choose a terminal, files, or both.', ja: 'ターミナルかファイルのどちらかを選んでください。' },
  'remote.proto.telnetSftp': {
    ko: 'SFTP는 SSH 위에서 동작합니다. 터미널을 SSH로 하거나 없음으로 고르세요.',
    en: 'SFTP runs over SSH: choose SSH for the terminal, or none.',
    ja: 'SFTP は SSH 上で動作します。ターミナルを SSH にするか、なしを選んでください。',
  },
  'remote.proto.only': { ko: '{name} 전용', en: '{name} only', ja: '{name} のみ' },
  'remote.proto.filter': { ko: '모든 프로토콜', en: 'All protocols', ja: 'すべてのプロトコル' },
  'remote.proto.plain': { ko: '암호화되지 않는 연결', en: 'Unencrypted connection', ja: '暗号化されない接続' },
  'remote.menuFiles': { ko: '파일 열기', en: 'Open files', ja: 'ファイルを開く' },
  'remote.status.partial': { ko: '일부만 됨', en: 'Partly reachable', ja: '一部のみ' },
  'remote.checks': { ko: '접속 방식별 결과', en: 'By protocol', ja: 'プロトコル別の結果' },
  'remote.noShellStats': {
    ko: 'CPU, 메모리 같은 정보는 SSH 셸로 접속하는 서버에서만 볼 수 있습니다.',
    en: 'CPU, memory and the like are shown only for servers with an SSH shell.',
    ja: 'CPU やメモリなどの情報は SSH シェルで接続するサーバーでのみ表示されます。',
  },
  'remote.detect.title': { ko: '접속 방식 확인', en: 'Find out what works', ja: '接続方式の確認' },
  'remote.detect.hint': {
    ko: 'SSH 셸, SFTP, FTP(TLS 포함)가 각각 되는지 시험해 보고 결과에 맞게 접속 방식을 채웁니다.',
    en: 'Tries the SSH shell, SFTP and FTP (with TLS) one by one, and sets the protocols to what works.',
    ja: 'SSH シェル、SFTP、FTP (TLS を含む) がそれぞれ使えるか試し、結果に合わせて接続方式を設定します。',
  },
  'remote.detect.run': { ko: '확인하기', en: 'Check', ja: '確認する' },
  'remote.detect.running': { ko: '확인 중…', en: 'Checking…', ja: '確認中…' },
  'remote.detect.applied': {
    ko: '확인 결과에 맞춰 접속 방식을 {result}(으)로 바꿨습니다. 다르면 아래 서버 설정에서 고치세요.',
    en: 'Set the protocols to {result}, as the check found. Correct them below if that is wrong.',
    ja: '確認結果に合わせて接続方式を {result} に変更しました。違う場合は下のサーバー設定で直してください。',
  },
  'remote.detect.same': { ko: '설정된 접속 방식({result})이 확인 결과와 같습니다.', en: 'The protocols ({result}) match what works.', ja: '設定済みの接続方式 ({result}) は確認結果と同じです。' },
  'remote.detect.noneWorked': {
    ko: '되는 접속 방식을 찾지 못해 설정을 그대로 두었습니다. 아래 결과에서 원인을 확인하세요.',
    en: 'Nothing worked, so the settings were left as they are. See why below.',
    ja: '使える接続方式が見つからなかったため、設定はそのままです。下の結果で原因を確認してください。',
  },
};

export default t;
