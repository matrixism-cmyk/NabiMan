export default {
  'processes.scopeHelp': { ko: '본 서버의 CPU 사용률 상위 200개 표본 중 최대 100개를 표시합니다. 읽을 수 없거나 사라진 프로세스는 제외됩니다. CPU·메모리 표본과 대상 정보는 동시에 수집되지 않습니다.', en: 'Up to 100 rows from the top 200 local CPU samples. Unreadable or vanished processes are omitted. Resource samples and process identities are not captured atomically.', ja: '本サーバーのCPU上位200標本から最大100件を表示します。取得不可・消滅したプロセスは除外し、指標と識別情報は同時取得ではありません。' },
  'processes.readOnly': { ko: '신호 전달에는 최고관리자 권한과 pidfd 지원이 필요합니다.', en: 'Sending signals requires the highest administrator and pidfd support.', ja: 'シグナル送信には最高管理者権限とpidfdサポートが必要です。' },
  'processes.termHelp': { ko: 'TERM은 정상 종료를 요청합니다. 프로세스가 무시하거나 종료가 지연될 수 있습니다.', en: 'TERM requests graceful termination. The process may ignore it or take time to exit.', ja: 'TERMは正常終了を要求します。無視されたり終了に時間がかかる場合があります。' },
  'processes.killHelp': { ko: 'KILL은 정리 작업 없이 강제 종료합니다. 저장하지 않은 데이터가 손실될 수 있습니다.', en: 'KILL terminates without application cleanup. Unsaved data may be lost.', ja: 'KILLは後処理なしで強制終了します。未保存データが失われる可能性があります。' },
  'processes.confirmPid': { ko: '표시된 PID를 입력해 대상 확인', en: 'Type the displayed PID to confirm the target', ja: '表示されたPIDを入力して対象を確認' },
  'processes.send': { ko: '확인한 대상에 신호 보내기', en: 'Send signal to reviewed process', ja: '確認した対象にシグナルを送信' },
  'processes.stale': { ko: '대상이 변경되었거나 사라졌습니다. 다시 선택하세요.', en: 'The target changed or disappeared. Select it again.', ja: '対象が変更または消滅しました。再選択してください。' },
  'processes.exited': { ko: '신호를 전달했고 해당 프로세스의 종료를 관찰했습니다.', en: 'Signal accepted; exit of the referenced process was observed.', ja: 'シグナルが受理され、対象プロセスの終了を確認しました。' },
  'processes.present': { ko: '신호는 전달했지만 관찰 시간 내 종료를 확인하지 못했습니다.', en: 'Signal accepted; no exit was observed within the observation window.', ja: 'シグナルは受理されましたが、観測時間内に終了を確認できませんでした。' },
  'processes.unknownObservation': { ko: '신호는 전달했지만 이후 상태를 확인하지 못했습니다.', en: 'Signal accepted; subsequent process state could not be observed.', ja: 'シグナルは受理されましたが、その後の状態を確認できませんでした。' },
  'processes.unknownResult': { ko: '전달 결과를 확인하지 못했습니다. 자동 재시도하지 않습니다. 현재 상태를 다시 확인하세요.', en: 'Signal outcome was not confirmed. No automatic retry. Inspect the current process state.', ja: '送信結果を確認できません。自動再試行せず、現在の状態を確認してください。' },
};
