import type { Translations } from '../../shared/i18n';

// Plans, limits and grace (licences that carry `limits`).
const t: Translations = {
  'license.state.grace': { ko: '라이선스가 만료되어 유예 기간입니다.', en: 'The license has expired; grace period.', ja: 'ライセンスの期限が切れ、猶予期間中です。' },
  'license.plan': { ko: '요금제', en: 'Plan', ja: 'プラン' },
  'license.deployment': { ko: '제공 방식', en: 'Deployment', ja: '提供形態' },
  'license.deployment.installed': { ko: '설치형', en: 'Installed', ja: 'インストール型' },
  'license.deployment.hosted': { ko: '호스팅', en: 'Hosted', ja: 'ホスティング' },
  'license.maxUsers': { ko: '사용자 (사용 / 한도)', en: 'Users (in use / limit)', ja: 'ユーザー（使用 / 上限）' },
  'license.planServers': { ko: '서버 (사용 / 한도)', en: 'Servers (in use / limit)', ja: 'サーバー（使用 / 上限）' },
  'license.noLimit': { ko: '제한 없음', en: 'No limit', ja: '上限なし' },
  'license.planFeatures': { ko: '요금제 기능', en: 'Plan features', ja: 'プラン機能' },
  'license.graceEnds': { ko: '유예 종료일', en: 'Grace ends', ja: '猶予終了日' },
  'license.feature.orchestration': { ko: 'Claude 지휘·연결 보기', en: 'Claude orchestration view', ja: 'Claude 指揮・連携ビュー' },
  'license.feature.graph_3d': { ko: '3D 작업 지도', en: '3D work map', ja: '3D 作業マップ' },
  'license.feature.recording': { ko: '세션 녹화', en: 'Session recording', ja: 'セッション録画' },
  'license.feature.mec': { ko: 'MEC 관리', en: 'MEC management', ja: 'MEC 管理' },
  'license.notice.grace': {
    ko: '라이선스가 만료되었습니다. 유예 기간이 {date}까지이며, 그 뒤에는 새 사용자·서버를 추가할 수 없습니다. 기존 데이터와 기능은 그대로입니다.',
    en: 'The license has expired. The grace period runs until {date}; after that no new users or servers can be added. Existing data and features stay as they are.',
    ja: 'ライセンスの期限が切れました。猶予期間は{date}までで、その後は新しいユーザー・サーバーを追加できません。既存のデータと機能はそのままです。',
  },
  'license.notice.grace_ended': {
    ko: '라이선스 유예 기간이 끝나 새 사용자·서버를 추가할 수 없습니다. 기존 데이터와 기능은 그대로 쓸 수 있습니다. 라이선스를 갱신하세요.',
    en: 'The grace period has ended: no new users or servers can be added. Existing data and features keep working. Renew the license.',
    ja: '猶予期間が終了したため、新しいユーザー・サーバーを追加できません。既存のデータと機能は引き続き使えます。ライセンスを更新してください。',
  },
  'license.notice.expiring_soon': {
    ko: '라이선스가 {days}일 뒤({date}) 만료됩니다.',
    en: 'The license expires in {days} days ({date}).',
    ja: 'ライセンスは{days}日後（{date}）に期限が切れます。',
  },
  'license.notice.users_at_limit': {
    ko: '사용자가 라이선스 한도({max}명)에 도달했습니다. 더 추가하려면 라이선스를 늘리세요.',
    en: 'Users have reached the license limit ({max}). Extend the license to add more.',
    ja: 'ユーザー数がライセンス上限（{max}）に達しました。追加するにはライセンスを拡張してください。',
  },
  'license.notice.users_over_limit': {
    ko: '사용자({count}명)가 라이선스 한도({max}명)를 넘었습니다. 기존 계정은 그대로이며 새 계정은 추가할 수 없습니다.',
    en: 'Users ({count}) exceed the license limit ({max}). Existing accounts are kept; no new ones can be added.',
    ja: 'ユーザー数（{count}）がライセンス上限（{max}）を超えています。既存アカウントはそのままで、新規追加はできません。',
  },
  'license.notice.servers_at_limit': {
    ko: '저장한 서버가 라이선스 한도({max}대)에 도달했습니다. 더 추가하려면 라이선스를 늘리세요.',
    en: 'Saved servers have reached the license limit ({max}). Extend the license to add more.',
    ja: '保存サーバー数がライセンス上限（{max}）に達しました。追加するにはライセンスを拡張してください。',
  },
  'license.notice.servers_over_limit': {
    ko: '저장한 서버({count}대)가 라이선스 한도({max}대)를 넘었습니다. 기존 서버는 그대로이며 새 서버는 추가할 수 없습니다.',
    en: 'Saved servers ({count}) exceed the license limit ({max}). Existing servers are kept; no new ones can be added.',
    ja: '保存サーバー数（{count}）がライセンス上限（{max}）を超えています。既存サーバーはそのままで、新規追加はできません。',
  },
  'license.banner.open': { ko: '라이선스 보기', en: 'View license', ja: 'ライセンスを表示' },
  'license.banner.label': { ko: '라이선스 알림', en: 'License notice', ja: 'ライセンスのお知らせ' },
};

export default t;
