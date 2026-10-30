# Niki

Windows 및 macOS(MacBook)용 Rust/Tauri 일기 앱입니다. 구독, 광고, 별도 계정 없이 로컬에서 사용할 수 있으며, 선택 시 사용자의 Google Drive(`appDataFolder`)를 통해 암호화 동기화를 지원합니다. 동기화 키는 같은 Google Drive 계정에 보관됩니다. [Google Drive 연동 설계](docs/GOOGLE_DRIVE_DESIGN.md)를 제공합니다.

## 디렉토리 구조

```text
niki/
├── capabilities/          # Tauri 권한 설정
├── docs/                  # 기획, 설계, 보안 검토 및 OAuth 설정 문서
├── icons/                 # 앱 실행 아이콘 (ICO, ICNS, PNG)
├── scripts/               # macOS 앱 번들(.app) 및 DMG 빌드 스크립트
├── src/                   # Rust 백엔드 소스 코드
│   ├── archive/           # ZIP 아카이브 리더 (zip_read.rs)
│   ├── drive/             # Google OAuth 인증 및 Drive API 전송 (auth.rs, transport.rs)
│   ├── installer/         # Windows/macOS 설치·제거 및 바로가기/Applications 등록 (mod.rs)
│   ├── sync/              # AES-256-GCM 암호화, 미디어 청크, 3-way 병합 엔진 (crypto.rs, engine.rs, media.rs, merge.rs)
│   ├── updater/           # Ed25519/SHA-256 검증 기반 자동 업데이터 (mod.rs)
│   ├── lib.rs             # Tauri 명령 핸들러 및 로컬 저장소 관리
│   └── main.rs            # 엔트리포인트
├── tests/                 # 테스트 픽스처
├── ui/                    # 프론트엔드 UI (index.html, app.js, style.css, icon.svg)
├── build.rs               # 빌드 스크립트 (OAuth 및 업데이터 설정 임베딩)
├── Cargo.toml             # Rust 패키지 매니페스트
├── tauri.conf.json        # Tauri 앱 설정
└── updater-config.json    # 자동 업데이트 엔드포인트 설정
```

## 실행 및 빌드

### Windows

Rust 1.99 이상과 Windows WebView2 런타임이 필요합니다.

```powershell
cargo run --offline
```

배포용 실행 파일:

```powershell
cargo build --release --offline
```

실행 파일은 `target/release/niki_journal.exe` (`Niki_Setup.exe`)입니다.

### macOS (MacBook — Apple Silicon & Intel)

macOS 기본 WebKit을 사용하며, 로컬 개발 실행 및 `.app` / `.dmg` 패키징을 지원합니다.

```bash
# 개발 실행
cargo run

# 현재 Mac 아키텍처용 Niki.app 및 Niki_macOS.dmg 빌드
./scripts/build-macos.sh

# Apple Silicon + Intel 통합 유니버설 바이너리 DMG 빌드
./scripts/build-macos.sh --universal
```

빌드 완료 시 `target/release/Niki.app` 및 `target/release/Niki_macOS.dmg`가 생성됩니다. 앱 데이터는 각 운영체제의 사용자별 앱 데이터 폴더에 저장되며, 개발·테스트에서는 `NIKI_JOURNAL_DATA_DIR` 환경 변수로 저장 위치를 바꿀 수 있습니다.

> **macOS 첫 실행 시 보안 경고가 나타나는 경우:**
> Apple Developer 유료 인증서로 공증되지 않은 오픈소스 앱은 macOS Gatekeeper에 의해 첫 실행 시 차단될 수 있습니다. Finder의 **응용 프로그램** 폴더에서 `Niki.app`을 **Control 키를 누른 채 클릭(우클릭) > [열기]**를 누르거나, 터미널에서 `xattr -cr /Applications/Niki.app`을 실행하면 정상 실행됩니다.

## 현재 제공 기능

- 여러 일기장(우클릭 수정·삭제), 글 작성·수정·휴지통·휴지통 비우기·복원, 자동 저장
- 좌측 상단 사용자 별명 표시 (미설정 시 `안녕하세요!`) 및 메뉴바 접기/열기
- 날짜, 제목, 서식이 바로 보이는 본문, 태그, 즐겨찾기, 장소, 날씨 메모
- 타임라인·달력·미디어·이 날의 기록, 키워드 및 태그 검색
- 사진·영상·오디오·PDF 첨부와 미리보기
- 사용자 템플릿, 앱 화면 잠금
- JSON/ZIP 가져오기, 첨부를 포함한 자체 JSON 백업
- 인쇄 대화상자를 통한 PDF 저장
- Google Drive(`appDataFolder`) AES-256-GCM 암호화 동기화 및 Google Drive 내 키 자동 보관, 3-way 병합 및 충돌 해결
- GitHub Releases 및 자체 웹 서버(`latest.json`) 기반 자동 업데이트 알림 및 원클릭 설치

## 보안 및 문서

- [Google Drive 동기화 설계](docs/GOOGLE_DRIVE_DESIGN.md)
- [Google OAuth 설정 안내](docs/GOOGLE_OAUTH_SETUP.md)
- [보안 재검토](docs/SECURITY_REVIEW.md)

## 검증

```powershell
cargo test --offline
node --check ui/app.js
```

에디터는 본문을 기존 Markdown 문자열 형식으로 저장합니다. 기존 기록은 본문을 수정하기 전까지 원문을 그대로 유지하며, 표·이미지·HTML 등 지원하지 않는 형식은 손실을 막기 위해 원문 편집기로 표시합니다. 다른 기기에서도 동일한 편집 경험을 사용하려면 Niki를 최신 버전으로 업데이트하세요.

Drive 동기화는 앱 실행 중 저장 후 약 8초, 시작·잠금 해제·네트워크 복구 시, 그리고 Rust 백그라운드 타이머로 5분마다 시도합니다. 앱을 종료한 뒤에는 동기화되지 않습니다. 업로드 전 리비전을 로컬에 저장해 업로드와 체크포인트 저장이 엇갈려도 같은 리비전으로 재시도합니다. 일지·템플릿 삭제는 삭제 리비전으로 전달하며, 동시에 수정된 일지·템플릿은 복사본을 남깁니다. 해독·검증에 실패한 원격 파일은 자동으로 버리지 않고 동기화를 멈춥니다. 설정에서 백업을 확인한 후 해당 파일을 명시적으로 건너뛰거나 나중에 다시 확인할 수 있습니다. 새 동기화 동작을 모든 기기에서 사용하려면 모든 기기를 최신 버전으로 업데이트하세요.

프런트엔드 편집기를 변경할 때는 Node.js 22 이상에서 `npm ci`, `npm run build`, `npm test`를 실행합니다. 빌드된 `ui/rich-editor.js`는 Rust 빌드에 포함되며, 의존성 라이선스는 `ui/THIRD_PARTY_LICENSES.txt`에 있습니다. 브라우저 테스트는 Chrome이 필요하며 `CHROME_PATH`로 경로를 지정할 수 있습니다.
