import { Lang } from './index';

// MEC firewall (AXGATE NAT + public IP) and Ingress panel copy. Reuses the
// shared mec.action.* / mec.state.* / mec.col.* keys defined in mec-live.ts.
const mecFirewall: Record<string, Record<Lang, string>> = {
  // ── Firewall panel ──────────────────────────────────────────────────────
  'mec.fw.title': { ko: 'AXGATE 방화벽', en: 'AXGATE Firewall', ja: 'AXGATE ファイアウォール' },
  'mec.fw.subtitle': {
    ko: 'NAT {active}/{total} 활성 · 공인 IP 할당 {assigned} · 가용 {available}',
    en: 'NAT {active}/{total} active · Public IP assigned {assigned} · available {available}',
    ja: 'NAT {active}/{total} 有効 · パブリックIP割当 {assigned} · 空き {available}',
  },
  'mec.fw.addRule': { ko: '+ NAT 규칙', en: '+ NAT Rule', ja: '+ NAT ルール' },
  'mec.fw.cancel': { ko: '취소', en: 'Cancel', ja: 'キャンセル' },

  'mec.fw.natRulesCard': { ko: 'NAT 규칙', en: 'NAT Rules', ja: 'NAT ルール' },
  'mec.fw.natInactive': { ko: '{count}개 비활성', en: '{count} inactive', ja: '{count}件 無効' },
  'mec.fw.publicIpCard': { ko: '공인 IP 할당', en: 'Public IP Assigned', ja: 'パブリックIP割当' },
  'mec.fw.ipAvailable': { ko: '{count}개 가용', en: '{count} available', ja: '{count}件 空き' },

  // Public IP section
  'mec.fw.publicIpSection': { ko: '공인 IP', en: 'Public IPs', ja: 'パブリックIP' },
  'mec.fw.ipFilter': { ko: '필터 (IP, 상태)', en: 'Filter (IP, status)', ja: 'フィルター (IP, 状態)' },
  'mec.fw.noPublicIp': { ko: '공인 IP 정보가 없습니다.', en: 'No public IP information.', ja: 'パブリックIP情報がありません。' },
  'mec.fw.colStatus': { ko: '상태', en: 'Status', ja: '状態' },
  'mec.fw.colUsage': { ko: '용도', en: 'Usage', ja: '用途' },
  'mec.fw.systemReserved': { ko: '시스템 예약', en: 'System reserved', ja: 'システム予約' },
  'mec.fw.assign': { ko: '할당', en: 'Assign', ja: '割当' },
  'mec.fw.release': { ko: '해제', en: 'Release', ja: '解除' },
  'mec.fw.releasing': { ko: '해제 중...', en: 'Releasing…', ja: '解除中…' },

  // NAT rules section
  'mec.fw.natRulesSection': { ko: 'NAT 규칙', en: 'NAT Rules', ja: 'NAT ルール' },
  'mec.fw.natFilter': { ko: '필터 (ID, label, IP, zone)', en: 'Filter (ID, label, IP, zone)', ja: 'フィルター (ID, label, IP, zone)' },
  'mec.fw.noNatRules': { ko: 'NAT 규칙이 없습니다.', en: 'No NAT rules.', ja: 'NAT ルールがありません。' },
  'mec.fw.colPublicIp': { ko: '공인 IP', en: 'Public IP', ja: 'パブリックIP' },
  'mec.fw.delete': { ko: '삭제', en: 'Delete', ja: '削除' },

  // Release / delete prompts and messages
  'mec.fw.releaseNoRule': {
    ko: '공인 IP {ip} 에 매핑된 NAT 규칙을 찾지 못했습니다.',
    en: 'No NAT rule mapped to public IP {ip} was found.',
    ja: 'パブリックIP {ip} にマッピングされた NAT ルールが見つかりませんでした。',
  },
  'mec.fw.releasePromptTitle': { ko: '⚠️ 공인 IP \'{ip}\' 할당 해제', en: '⚠️ Release public IP \'{ip}\'', ja: '⚠️ パブリックIP \'{ip}\' の割当解除' },
  'mec.fw.releasePromptRules': {
    ko: '연결된 NAT 규칙 {count}건이 삭제됩니다:',
    en: '{count} associated NAT rule(s) will be deleted:',
    ja: '関連する NAT ルール {count}件 が削除されます:',
  },
  'mec.fw.releasePromptProxyArp': {
    ko: 'proxy-arp 도 같이 해제됩니다 (AXGATE add_nat_rule delete 흐름).',
    en: 'proxy-arp will be released as well (AXGATE add_nat_rule delete flow).',
    ja: 'proxy-arp も併せて解除されます (AXGATE add_nat_rule delete フロー)。',
  },
  'mec.fw.releasePromptConfirm': {
    ko: '확인을 위해 IP 를 다시 입력하세요:',
    en: 'Re-enter the IP to confirm:',
    ja: '確認のため IP を再入力してください:',
  },
  'mec.fw.releaseMismatch': {
    ko: '입력 IP 가 일치하지 않아 해제 취소.',
    en: 'Entered IP does not match — release cancelled.',
    ja: '入力した IP が一致しないため解除をキャンセルしました。',
  },
  'mec.fw.deletePrompt': {
    ko: '⚠️ NAT 규칙 \'{id}\' 삭제 (외부 접근 끊길 수 있음). ID 재입력:',
    en: '⚠️ Delete NAT rule \'{id}\' (external access may be lost). Re-enter ID:',
    ja: '⚠️ NAT ルール \'{id}\' を削除 (外部アクセスが切断される場合があります)。ID を再入力:',
  },
  'mec.fw.deleteMismatch': {
    ko: '입력 ID 가 일치하지 않아 삭제 취소.',
    en: 'Entered ID does not match — deletion cancelled.',
    ja: '入力した ID が一致しないため削除をキャンセルしました。',
  },

  // ── NAT rule form ───────────────────────────────────────────────────────
  'mec.fw.formTitle': { ko: 'NAT 규칙 추가', en: 'Add NAT Rule', ja: 'NAT ルール追加' },
  'mec.fw.formLabelPlaceholder': { ko: '예: app-service-443', en: 'e.g. app-service-443', ja: '例: app-service-443' },
  'mec.fw.formPublicIp': {
    ko: '공인 IP (가용 {count}개)',
    en: 'Public IP ({count} available)',
    ja: 'パブリックIP (空き {count}件)',
  },
  'mec.fw.formSelectFromList': { ko: '목록에서 선택', en: 'Select from list', ja: 'リストから選択' },
  'mec.fw.formEnterManually': { ko: '직접 입력', en: 'Enter manually', ja: '手動入力' },
  'mec.fw.formSelectAvailableIp': { ko: '-- 가용 IP 선택 --', en: '-- Select available IP --', ja: '-- 空きIPを選択 --' },
  'mec.fw.formNoAvailableIp': {
    ko: '가용 IP 없음 — 직접 입력 이용',
    en: 'No available IP — use manual entry',
    ja: '空きIPなし — 手動入力を使用',
  },
  'mec.fw.formPrivateIp': { ko: '내부 IP', en: 'Internal IP', ja: '内部IP' },
  'mec.fw.formProtocol': { ko: '프로토콜', en: 'Protocol', ja: 'プロトコル' },
  'mec.fw.formPorts': { ko: '포트 (쉼표 구분)', en: 'Ports (comma-separated)', ja: 'ポート (カンマ区切り)' },
  'mec.fw.formProxyArp': {
    ko: 'proxy-arp 생성 (AXGATE 외부 광고)',
    en: 'Create proxy-arp (AXGATE external advertisement)',
    ja: 'proxy-arp 作成 (AXGATE 外部広告)',
  },
  'mec.fw.formEnableImmediately': { ko: '즉시 활성화', en: 'Enable immediately', ja: '即時有効化' },
  'mec.fw.formApplying': { ko: '적용 중...', en: 'Applying…', ja: '適用中…' },
  'mec.fw.formAdd': { ko: '추가', en: 'Add', ja: '追加' },
  'mec.fw.formPublicIpRequired': {
    ko: '공인 IP 를 선택하거나 입력하세요.',
    en: 'Select or enter a public IP.',
    ja: 'パブリックIP を選択するか入力してください。',
  },

  // ── Ingress panel ───────────────────────────────────────────────────────
  'mec.ingress.title': { ko: 'Ingress 규칙', en: 'Ingress Rules', ja: 'Ingress ルール' },
  'mec.ingress.subtitle': { ko: '{count}개 규칙', en: '{count} rules', ja: '{count}件 のルール' },
  'mec.ingress.add': { ko: '+ Ingress', en: '+ Ingress', ja: '+ Ingress' },
  'mec.ingress.cancel': { ko: '취소', en: 'Cancel', ja: 'キャンセル' },

  'mec.ingress.formTitle': { ko: 'Ingress 추가', en: 'Add Ingress', ja: 'Ingress 追加' },
  'mec.ingress.name': { ko: '이름', en: 'Name', ja: '名前' },
  'mec.ingress.tlsSecret': { ko: 'TLS Secret (선택)', en: 'TLS Secret (optional)', ja: 'TLS Secret (任意)' },
  'mec.ingress.creating': { ko: '생성 중...', en: 'Creating…', ja: '作成中…' },
  'mec.ingress.create': { ko: '생성', en: 'Create', ja: '作成' },

  'mec.ingress.filter': {
    ko: '필터 (host, backend, namespace)',
    en: 'Filter (host, backend, namespace)',
    ja: 'フィルター (host, backend, namespace)',
  },
  'mec.ingress.noRules': { ko: 'Ingress 규칙이 없습니다.', en: 'No Ingress rules.', ja: 'Ingress ルールがありません。' },

  'mec.ingress.deletePrompt': {
    ko: '⚠️ Ingress {ns}/{name} 을(를) 삭제합니다.\n확인을 위해 이름을 다시 입력하세요:',
    en: '⚠️ Deleting Ingress {ns}/{name}.\nRe-enter the name to confirm:',
    ja: '⚠️ Ingress {ns}/{name} を削除します。\n確認のため名前を再入力してください:',
  },
  'mec.ingress.deleteMismatch': {
    ko: '입력 이름이 일치하지 않아 삭제 취소.',
    en: 'Entered name does not match — deletion cancelled.',
    ja: '入力した名前が一致しないため削除をキャンセルしました。',
  },
  'mec.ingress.delete': { ko: '삭제', en: 'Delete', ja: '削除' },
};

export default mecFirewall;
