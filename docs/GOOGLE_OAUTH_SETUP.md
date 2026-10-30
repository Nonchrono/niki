# Google Drive OAuth 설정 준비

설정에서 Google 계정을 연결하면 동기화 키가 해당 계정의 Drive 앱 데이터 폴더에 자동 보관된다. 같은 계정에 접근할 수 있는 측은 키도 얻을 수 있으므로 종단간 암호화는 아니다.

1. [Google Cloud 콘솔](https://console.cloud.google.com/)에서 프로젝트를 만들거나 선택한다.
2. API 및 서비스 라이브러리에서 **Google Drive API**를 사용 설정한다.
3. Google Auth Platform에서 앱 이름, 지원 이메일, 대상 사용자, 연락처를 설정한다. 외부 사용자용 테스트 상태라면 테스트할 Google 계정을 테스트 사용자로 추가한다.
4. Data Access에서 `https://www.googleapis.com/auth/drive.appdata` 범위를 등록한다. 보이는 Drive 백업을 나중에 제공할 때에만 별도로 `https://www.googleapis.com/auth/drive.file`을 검토한다.
5. Clients에서 OAuth 클라이언트를 만들고 애플리케이션 유형으로 **Desktop app**을 선택한다. 발급된 클라이언트 ID를 보관한다. 데스크톱 앱에 들어가는 클라이언트 비밀은 기밀로 유지할 수 없으므로 보안 경계로 취급하지 않는다.
6. 앱 설정의 Google 계정 연결에 클라이언트 ID와 필요한 경우 클라이언트 비밀번호를 입력(또는 `NIKI_GOOGLE_CLIENT_ID`, `NIKI_GOOGLE_CLIENT_SECRET` 환경 변수로 지정)한다. 인증은 시스템 브라우저와 loopback 콜백, 요청마다 새로운 state 및 PKCE S256을 사용한다. 토큰과 클라이언트 비밀번호는 운영체제 자격 증명 저장소에 보관한다.
7. 다른 기기에서 같은 Google 계정을 연결하면 앱이 Drive에서 동기화 키를 자동으로 읽는다. 기존 형식에서 이전할 때는 모든 기기를 새 버전으로 업데이트한다.

외부 배포 시 동의 화면 게시 상태와 Google의 앱 검증 요건을 확인한다. 테스트 계정의 개인 기록을 사용하기 전에 별도 테스트 데이터를 사용하고 다른 위치에 백업을 보관한다.

공식 문서: [API 사용 설정](https://developers.google.com/workspace/guides/enable-apis), [동의 화면 설정](https://developers.google.com/workspace/guides/configure-oauth-consent), [데스크톱 OAuth](https://developers.google.com/identity/protocols/oauth2/native-app), [Drive 권한](https://developers.google.com/workspace/drive/api/guides/api-specific-auth).
