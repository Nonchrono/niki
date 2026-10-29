# Niki

Windows 및 macOS(MacBook)용 Rust/Tauri 일기 앱입니다. 구독, 광고, 별도 계정 없이 로컬에서 사용할 수 있으며, 선택 시 사용자의 Google Drive(`appDataFolder`)를 통해 종단간 암호화 동기화를 지원합니다. [기능 기획서](docs/DAY_ONE_FREE_APP_PLAN.md)와 [Google Drive 연동 설계](docs/GOOGLE_DRIVE_DESIGN.md)를 함께 제공합니다.

## 디렉토리 구조

```text
niki/
├── capabilities/          # Tauri 권한 설정
├── docs/                  # 기획, 설계, 보안 검토 및 OAuth 설정 문서
├── icons/                 # 앱 실행 아이콘 (ICO, ICNS, PNG)
├── scripts/               # macOS 앱 번들(.app) 및 DMG 빌드 스크립트
├── src/                   # Rust 백엔드 소스 코드
│   ├── archive/           # ZIP/Day One 아카이브 리더 (zip_read.rs)
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

## 현재 제공 기능

- 여러 일기장(우클릭 수정·삭제), 글 작성·수정·휴지통·휴지통 비우기·복원, 자동 저장
- 좌측 상단 사용자 별명 표시 (미설정 시 `안녕하세요!`) 및 메뉴바 접기/열기
- 날짜, 제목, Markdown 본문, 태그, 즐겨찾기, 장소, 날씨 메모
- 타임라인·달력·미디어·이 날의 기록, 키워드 및 태그 검색
- 사진·영상·오디오·PDF 첨부와 미리보기
- 사용자 템플릿, 앱 화면 잠금
- Day One JSON/ZIP 가져오기, 첨부를 포함한 자체 JSON 백업
- 인쇄 대화상자를 통한 PDF 저장
- Google Drive(`appDataFolder`) 종단간 암호화(AES-256-GCM) 동기화, 복구 키 백업/복원, 3-way 병합 및 충돌 해결
- GitHub Releases 및 자체 웹 서버(`latest.json`) 기반 자동 업데이트 알림 및 원클릭 설치

## 보안 및 문서

- [기능 기획서](docs/DAY_ONE_FREE_APP_PLAN.md)
- [Google Drive 동기화 설계](docs/GOOGLE_DRIVE_DESIGN.md)
- [Google OAuth 설정 안내](docs/GOOGLE_OAUTH_SETUP.md)
- [보안 재검토](docs/SECURITY_REVIEW.md)

## 검증

```powershell
cargo test --offline
node --check ui/app.js
```

이 앱은 Day One의 상표·코드·서버를 사용하지 않는 독립 구현입니다.
