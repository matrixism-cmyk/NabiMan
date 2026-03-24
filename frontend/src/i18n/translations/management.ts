type Lang = 'ko' | 'en' | 'ja';
type T = Record<string, Record<Lang, string>>;

const t: T = {
  // --- Config ---
  'config.title': { ko: '서비스 설정', en: 'Service Configuration', ja: 'サービス設定' },
  'config.installed': { ko: '설치됨', en: 'Installed', ja: 'インストール済' },
  'config.running': { ko: '실행 중', en: 'Running', ja: '実行中' },
  'config.stopped': { ko: '정지', en: 'Stopped', ja: '停止' },
  'config.validate': { ko: '검증', en: 'Validate', ja: '検証' },
  'config.saving': { ko: '저장 중...', en: 'Saving...', ja: '保存中...' },
  'config.noServices': { ko: '서비스가 감지되지 않았습니다. \'전체\'를 클릭하세요.', en: 'No services detected. Try \'All\' to see all registered services.', ja: 'サービスが検出されませんでした。「すべて」をクリックしてください。' },
  'config.loading': { ko: '서비스 설정을 불러오는 중...', en: 'Loading service configurations...', ja: 'サービス設定を読み込み中...' },

  // --- Packages ---
  'packages.title': { ko: '패키지 관리', en: 'Package Management', ja: 'パッケージ管理' },
  'packages.installPlaceholder': { ko: '설치할 패키지 이름', en: 'Package name to install', ja: 'インストールするパッケージ名' },
  'packages.searchPlaceholder': { ko: '패키지 검색...', en: 'Search packages...', ja: 'パッケージを検索...' },
  'packages.searchResults': { ko: '검색 결과', en: 'Search Results', ja: '検索結果' },
  'packages.searching': { ko: '검색 중...', en: 'Searching...', ja: '検索中...' },
  'packages.installed': { ko: '설치된 패키지', en: 'Installed Packages', ja: 'インストール済みパッケージ' },
  'packages.filterPlaceholder': { ko: '설치된 패키지 필터...', en: 'Filter installed packages...', ja: 'インストール済みパッケージをフィルター...' },
  'packages.package': { ko: '패키지', en: 'Package', ja: 'パッケージ' },
  'packages.action': { ko: '작업', en: 'Action', ja: '操作' },
  'packages.loading': { ko: '패키지를 불러오는 중...', en: 'Loading packages...', ja: 'パッケージを読み込み中...' },

  // --- Containers ---
  'containers.title': { ko: '컨테이너 관리', en: 'Container Management', ja: 'コンテナ管理' },
  'containers.containers': { ko: '컨테이너', en: 'Containers', ja: 'コンテナ' },
  'containers.images': { ko: '이미지', en: 'Images', ja: 'イメージ' },
  'containers.image': { ko: '이미지', en: 'Image', ja: 'イメージ' },
  'containers.ports': { ko: '포트', en: 'Ports', ja: 'ポート' },
  'containers.logs': { ko: '로그', en: 'Logs', ja: 'ログ' },
  'containers.repository': { ko: '리포지토리', en: 'Repository', ja: 'リポジトリ' },
  'containers.tag': { ko: '태그', en: 'Tag', ja: 'タグ' },
  'containers.id': { ko: 'ID', en: 'ID', ja: 'ID' },
  'containers.size': { ko: '크기', en: 'Size', ja: 'サイズ' },
  'containers.created': { ko: '생성일', en: 'Created', ja: '作成日' },
  'containers.noContainers': { ko: '컨테이너가 없습니다. Docker가 설치되지 않았거나 실행 중이 아닐 수 있습니다.', en: 'No containers found. Docker may not be installed or running.', ja: 'コンテナが見つかりません。Dockerがインストールされていないか、実行されていない可能性があります。' },
  'containers.loading': { ko: '컨테이너를 불러오는 중...', en: 'Loading containers...', ja: 'コンテナを読み込み中...' },

  // --- Services ---
  'services.title': { ko: '서비스 관리', en: 'Service Management', ja: 'サービス管理' },
  'services.filterPlaceholder': { ko: '서비스 필터...', en: 'Filter services...', ja: 'サービスをフィルター...' },
  'services.active': { ko: '활성', en: 'Active', ja: 'アクティブ' },
  'services.inactive': { ko: '비활성', en: 'Inactive', ja: '非アクティブ' },
  'services.failed': { ko: '실패', en: 'Failed', ja: '失敗' },
  'services.service': { ko: '서비스', en: 'Service', ja: 'サービス' },
  'services.state': { ko: '상태', en: 'State', ja: '状態' },
  'services.enabled': { ko: '활성화', en: 'Enabled', ja: '有効' },
  'services.enable': { ko: '활성화', en: 'Enable', ja: '有効化' },
  'services.disable': { ko: '비활성화', en: 'Disable', ja: '無効化' },
  'services.services': { ko: '서비스', en: 'services', ja: 'サービス' },
  'services.loading': { ko: '서비스를 불러오는 중...', en: 'Loading services...', ja: 'サービスを読み込み中...' },

  // --- Updates ---
  'updates.title': { ko: '시스템 업데이트', en: 'System Updates', ja: 'システムアップデート' },
  'updates.checkAgain': { ko: '다시 확인', en: 'Check Again', ja: '再確認' },
  'updates.upgradeAll': { ko: '모두 업그레이드', en: 'Upgrade All', ja: '一括更新' },
  'updates.upToDate': { ko: '시스템이 최신 상태입니다.', en: 'System is up to date.', ja: 'システムは最新です。' },
  'updates.package': { ko: '패키지', en: 'Package', ja: 'パッケージ' },
  'updates.current': { ko: '현재 버전', en: 'Current', ja: '現在' },
  'updates.available': { ko: '업데이트 버전', en: 'Available', ja: '利用可能' },
  'updates.confirmUpgrade': { ko: '모든 패키지를 업그레이드 하시겠습니까?', en: 'Upgrade all packages?', ja: 'すべてのパッケージを更新しますか？' },

  // --- Database ---
  'db.title': { ko: '데이터베이스', en: 'Database', ja: 'データベース' },
  'db.engine': { ko: '엔진', en: 'Engine', ja: 'エンジン' },
  'db.version': { ko: '버전', en: 'Version', ja: 'バージョン' },
  'db.connections': { ko: '연결 수', en: 'Connections', ja: '接続数' },
  'db.slowQueries': { ko: '슬로우 쿼리', en: 'Slow Queries', ja: 'スロークエリ' },
  'db.databases': { ko: '데이터베이스 목록', en: 'Databases', ja: 'データベース一覧' },

  // --- Backup ---
  'backup.title': { ko: '백업 관리', en: 'Backup Management', ja: 'バックアップ管理' },
  'backup.pathPlaceholder': { ko: '백업할 파일 경로 (예: /etc/nginx/nginx.conf)', en: 'File path to backup', ja: 'バックアップするファイルパス' },
  'backup.create': { ko: '백업 생성', en: 'Create Backup', ja: 'バックアップ作成' },
  'backup.restore': { ko: '복원', en: 'Restore', ja: '復元' },
  'backup.restoreTo': { ko: '복원 대상 경로를 입력하세요', en: 'Enter restore target path', ja: '復元先パスを入力' },
  'backup.confirmDelete': { ko: '이 백업을 삭제하시겠습니까?', en: 'Delete this backup?', ja: 'このバックアップを削除しますか？' },
  'backup.source': { ko: '원본', en: 'Source', ja: '元ファイル' },
  'backup.filename': { ko: '파일명', en: 'Filename', ja: 'ファイル名' },
  'backup.created': { ko: '생성일', en: 'Created', ja: '作成日' },
  'backup.noBackups': { ko: '백업이 없습니다', en: 'No backups', ja: 'バックアップなし' },

  // --- File Manager ---
  'files.title': { ko: '파일 관리자', en: 'File Manager', ja: 'ファイル管理' },
  'files.go': { ko: '이동', en: 'Go', ja: '移動' },
  'files.size': { ko: '크기', en: 'Size', ja: 'サイズ' },
  'files.perms': { ko: '권한', en: 'Perms', ja: '権限' },
  'files.modified': { ko: '수정일', en: 'Modified', ja: '更新日' },
  'files.saved': { ko: '파일이 저장되었습니다', en: 'File saved', ja: 'ファイルを保存しました' },
};

export default t;
