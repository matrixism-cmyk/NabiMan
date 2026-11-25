import { Lang } from '../context';

// Moving devices on the server-room rack page (components/rack/RackEditor).
const rackEdit: Record<string, Record<Lang, string>> = {
  'rack.edit.start': { ko: '위치 편집', en: 'Edit positions', ja: '位置を編集' },
  'rack.edit.banner': { ko: '편집 중 — 저장해야 반영됩니다', en: 'Editing — nothing changes until you save', ja: '編集中 — 保存するまで反映されません' },
  'rack.edit.hint': {
    ko: '장비를 끌어서 원하는 칸에 놓으세요. 키보드로는 장비를 고른 뒤 화살표 키로 옮깁니다(위·아래 한 칸, 좌·우 옆 랙).',
    en: 'Drag a device to the slot you want. With the keyboard, focus a device and use the arrow keys (up/down one slot, left/right to the next rack).',
    ja: '機器をドラッグして置きたい段に移します。キーボードでは機器を選び、矢印キーで動かします(上下で1段、左右で隣のラック)。',
  },
  'rack.edit.save': { ko: '저장', en: 'Save', ja: '保存' },
  'rack.edit.saveCount': { ko: '저장 ({count}대 이동)', en: 'Save ({count} moved)', ja: '保存({count}台移動)' },
  'rack.edit.saving': { ko: '저장하는 중…', en: 'Saving…', ja: '保存中…' },
  'rack.edit.undo': { ko: '되돌리기', en: 'Undo', ja: '元に戻す' },
  'rack.edit.cancel': { ko: '취소', en: 'Cancel', ja: 'キャンセル' },
  'rack.edit.cancelConfirm': {
    ko: '옮긴 장비 {count}대를 저장하지 않았습니다. 저장하지 않고 편집을 끝낼까요?',
    en: '{count} moved device(s) are not saved. Stop editing without saving?',
    ja: '移動した機器 {count}台がまだ保存されていません。保存せずに編集を終了しますか?',
  },
  'rack.edit.leaveConfirm': {
    ko: '저장하지 않은 위치 변경이 있습니다. 이 화면을 떠나면 사라집니다. 떠날까요?',
    en: 'There are position changes you have not saved. Leaving this page loses them. Leave?',
    ja: '保存していない位置の変更があります。この画面を離れると失われます。離れますか?',
  },
  'rack.edit.discard': { ko: '저장하지 않고 끝내기', en: 'Discard changes', ja: '保存せずに終了' },
  'rack.edit.moved': { ko: '이동됨', en: 'Moved', ja: '移動済み' },
  'rack.edit.origin': { ko: '{label}의 원래 자리', en: 'Where {label} was', ja: '{label} の元の位置' },
  'rack.edit.drop.outside': { ko: '랙 밖입니다', en: 'Outside the rack', ja: 'ラックの外です' },
  'rack.edit.drop.overlap': { ko: '{other} 자리와 겹칩니다', en: 'Overlaps {other}', ja: '{other} と重なります' },
  'rack.edit.said.moved': { ko: '{label}, {where}에 놓았습니다.', en: '{label} is now at {where}.', ja: '{label} を {where} に置きました。' },
  'rack.edit.said.outside': { ko: '{label}: 랙 밖이라 더 옮길 수 없습니다.', en: '{label}: cannot move further, that is outside the rack.', ja: '{label}: ラックの外になるため、これ以上動かせません。' },
  'rack.edit.said.overlap': { ko: '{label}: {other} 자리와 겹쳐서 옮길 수 없습니다.', en: '{label}: cannot move there, it would overlap {other}.', ja: '{label}: {other} と重なるため動かせません。' },
  'rack.edit.said.edge': { ko: '{label}: 그쪽에는 랙이 없습니다.', en: '{label}: there is no rack on that side.', ja: '{label}: そちら側にラックはありません。' },
  'rack.edit.said.unplaced': {
    ko: '{label}: 아직 칸이 정해지지 않은 장비입니다. 끌어서 랙에 놓아 주세요.',
    en: '{label}: this device has no slot yet. Drag it into a rack.',
    ja: '{label}: まだ段が決まっていない機器です。ドラッグしてラックに置いてください。',
  },
  'rack.edit.said.undone': { ko: '마지막 이동을 되돌렸습니다.', en: 'The last move was undone.', ja: '最後の移動を元に戻しました。' },
  'rack.edit.saved': { ko: '저장했습니다. 새 실장도 버전은 {version}입니다.', en: 'Saved. The new layout version is {version}.', ja: '保存しました。新しい実装図バージョンは {version} です。' },
  'rack.edit.conflict': {
    ko: '그사이 다른 사람이 새 버전({version})을 저장했습니다. 지금 옮긴 내용은 저장할 수 없습니다. 다시 불러오면 새 버전이 보이고, 지금 옮긴 내용은 사라집니다.',
    en: 'Someone else saved a new version ({version}) meanwhile, so these moves cannot be saved. Reloading shows the new version and discards the moves you made.',
    ja: 'その間に他の人が新しいバージョン({version})を保存しました。今の移動は保存できません。再読み込みすると新しいバージョンが表示され、今の移動は失われます。',
  },
  'rack.edit.reload': { ko: '다시 불러오기', en: 'Reload', ja: '再読み込み' },
  'rack.edit.error.not_configured': { ko: '서버실 랙 기능이 꺼져 있어 저장할 수 없습니다.', en: 'The rack page is turned off on the server, so nothing can be saved.', ja: 'ラック画面が無効のため保存できません。' },
  'rack.edit.error.layout_unusable': { ko: '랙 실장도 파일을 읽을 수 없어 저장하지 못했습니다.', en: 'Not saved: the layout file cannot be read.', ja: '実装図ファイルを読み込めないため保存できませんでした。' },
  'rack.edit.error.write_failed': {
    ko: '새 실장도 파일을 쓰지 못했습니다. 바뀐 것은 없습니다. 실장도 폴더에 쓸 수 있는지 확인해 주세요.',
    en: 'The new layout file could not be written. Nothing was changed. Check that the layout folder is writable.',
    ja: '新しい実装図ファイルを書き込めませんでした。変更はありません。実装図フォルダーに書き込めるか確認してください。',
  },
  'rack.edit.error.invalid_move': { ko: '서버가 이 위치를 받아들이지 않았습니다. 바뀐 것은 없습니다.', en: 'The server did not accept these positions. Nothing was changed.', ja: 'サーバーがこの位置を受け付けませんでした。変更はありません。' },
  'rack.edit.error.other': { ko: '저장하지 못했습니다. 바뀐 것은 없습니다.', en: 'Not saved. Nothing was changed.', ja: '保存できませんでした。変更はありません。' },
};

export default rackEdit;
