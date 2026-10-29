# 무료 데스크톱 일기 앱 기획서

작성 기준: 2026-10-29 · 대상: Windows 데스크톱 · 상태: 기능 조사 기반 초안

## 1. 목표와 범위

Day One Windows 앱의 일기 작성·탐색·보관 경험과 유료 기능을 **과금·광고·사용량 잠금 없이** 제공하는 독립 앱을 만든다. 사용자는 계정 없이 로컬에서 시작할 수 있어야 하며, 일지와 첨부 파일은 사용자 소유의 내보내기 가능한 형식으로 보관한다. 기능 기준은 현재 Windows 앱으로 잡는다. iOS 전용 기능(Apple Pencil 등)이나 실물 책 인쇄는 Windows 앱의 동등 기능으로 간주하지 않는다.

"완전 무료"는 앱의 라이선스, 기능, 사용량, 자체 서버 이용에 대한 비용 청구가 없다는 뜻이다. 사용자가 선택한 클라우드 저장소, 문자 발송 서비스, 인쇄 업체의 외부 비용까지 없다고 약속할 수는 없다. 따라서 핵심 기능은 외부 서비스 없이 작동하도록 설계하고, 서버가 필요한 기능은 자체 호스팅 또는 사용자가 보유한 저장소를 선택할 수 있게 한다.

## 2. 조사 결과와 확실성

| 구분 | 확인된 내용 | 근거와 한계 |
| --- | --- | --- |
| 설치 앱 | `22490Automattic.DayOneJournalPrivateDiary` Windows 패키지 1.1.1.0이 설치되어 있다. | 로컬 패키지 조회. 앱 창 실행은 두 차례 실패하여 실제 조작 화면은 검증하지 못했다. |
| 정적 구조 | REA 6.4.0 분석에서 Electron 패키지 `dayone-desktop`, 메인/프리로드, 일지 전체·미디어·달력 경로, 28개 리터럴 IPC 채널을 관찰했다. `create-new-entry`, `new-journal`, `new-shared-journal`, `manage-templates`, `password:*` 등의 채널이 있다. | 설치 파일 해시 `45c3f889…2250601`, REA Evidence ID `ev_22d46586d3af5e8aed245d2669b3e7016a983af8435c73a5f6ec12d6f0f78c7d`. 정적 구문에 나타난 후보이며 실행 동작을 입증하지 않는다. 메인 번들의 의미 분석 일부가 힙 한도로 중단됐다. |
| Windows 제품 구조 | 공식 설명상 Windows 앱은 웹 앱의 기능을 싣고 로컬 우선 저장과 오프라인 접근을 제공한다. Windows 앱은 타임라인·미디어·달력 보기이며 지도 보기는 없다. | [Windows 시작 안내](https://dayoneapp.com/guides/day-one-windows/day-one-for-windows-getting-started/), [웹 앱 시작 안내](https://dayoneapp.com/guides/day-one-on-the-web/getting-started-on-the-web/). |
| 요금제 | Day One은 Basic/Silver/Gold로 기능을 구분한다. 영상·오디오·PDF 첨부, 다중 기기 동기화, AI 기능 등이 상위 등급에 있다. | [공식 요금 및 기능표](https://dayoneapp.com/guides/premium-subscription/day-one-pricing-features-guide/). 요금과 구성은 변경될 수 있다. |

**해석:** 설치 패키지의 채널명과 공식 설명은 기능 목록을 만드는 데 충분하지만, 화면별 세부 동작·오류 처리·단축키까지 동일하다고 단정할 수 없다. 구현 전 테스트용 계정으로 Windows 앱을 실행해 시나리오별로 재검증한다. 개인 일기 데이터나 암호화 키는 분석 대상으로 사용하지 않는다.

## 3. 기능 요구사항

표의 우선순위는 출시 순서이며, **최종 목표는 모든 행의 제공**이다. 유료 등급은 만들지 않는다.

| 영역 | 요구 기능과 완료 기준 | 순서 | 근거 |
| --- | --- | --- | --- |
| 일지·글 | 일지 생성·이름/표지/색 변경·정렬·보관, 무제한 글 작성·수정·삭제·복원. 작성 시각과 표시 날짜를 분리하고 과거/미래 날짜 및 종일 항목을 지원한다. 편집 중 자동 저장, 앱 종료 후 복구, 첨부 누락 없는 저장을 검증한다. | 1 | [글 작성](https://dayoneapp.com/guides/getting-started-with-day-one/creating-entries/), [날짜 변경](https://dayoneapp.com/guides/tips-and-tutorials/change-the-date-time-of-an-entry/) |
| 편집기 | 제목·본문, 굵게/기울임/강조, 제목, 인용, 코드, 목록·체크리스트, 링크, 템플릿 삽입을 제공한다. 서식이 재시작·내보내기·가져오기 후 유지되어야 한다. | 1 | [웹 앱 편집기](https://dayoneapp.com/guides/day-one-on-the-web/getting-started-on-the-web/), [템플릿](https://dayoneapp.com/guides/tips-and-tutorials/templates/) |
| 탐색 | 전체/일지별 타임라인, 달력, 미디어 격자, 날짜 이동, 즐겨찾기, 태그, 키워드 검색과 다중 필터를 제공한다. 검색에는 위치·날씨·활동·템플릿·첨부 유형 등 저장된 메타데이터를 쓴다. | 1 | [웹 앱 보기](https://dayoneapp.com/guides/day-one-on-the-web/getting-started-on-the-web/), [Windows 검색](https://dayoneapp.com/guides/tips-and-tutorials/search-in-day-one/), [태그](https://dayoneapp.com/guides/tips-and-tutorials/tagging-guide/) |
| 미디어·메타데이터 | 글 하나에 여러 사진·영상·오디오·PDF를 첨부하고 미리보기/재생/다운로드한다. 날짜·위치·날씨·즐겨찾기 메타데이터를 편집한다. 저장 공간 이외의 인위적 개수 제한을 두지 않는다. 파일 형식과 변환 정책을 명시한다. | 1–2 | [미디어 지원](https://dayoneapp.com/guides/getting-started-with-day-one/adding-media-in-day-one/), [Windows 시작 안내](https://dayoneapp.com/guides/day-one-windows/day-one-for-windows-getting-started/) |
| 회고·작성 습관 | 오늘의 질문, 사용자 템플릿과 일지별 기본 템플릿, 이 날의 기록, 일정 알림을 제공한다. 알림을 누르면 해당 날짜·일지의 작성 화면이 열린다. | 2 | [템플릿](https://dayoneapp.com/guides/tips-and-tutorials/templates/), [알림](https://dayoneapp.com/guides/tips-and-tutorials/reminders/) |
| 개인 정보·보안 | 로컬 데이터 암호화, 앱 암호 잠금·유휴/절전 시 자동 잠금, 암호화 백업, 복구 키 안내를 제공한다. 암호 분실 시 복구 가능 범위와 손실 위험을 명확히 알린다. | 1–2 | [Windows 암호 잠금 안내](https://forums.dayoneapp.com/forums/topic/windows-beta-needs-password-protection/), [Day One 암호화 설명](https://dayoneapp.com/guides/day-one-on-the-web/getting-started-on-the-web/) |
| 이동성 | Day One JSON ZIP 가져오기, 자체 JSON/Markdown/PDF 내보내기, 첨부 포함 백업·복원, 중복 식별과 부분 실패 보고를 제공한다. 원본을 수정하지 않고 가져오기 미리보기와 취소를 지원한다. | 1 | [Windows 내보내기](https://dayoneapp.com/guides/tips-and-tutorials/exporting-entries/), [가져오기](https://dayoneapp.com/guides/settings/importing-data-to-day-one/) |
| 동기화·공유 | 여러 기기 간 암호화 동기화, 오프라인 변경의 충돌 처리, 공유 일지의 초대·승인·권한·댓글·반응·알림을 제공한다. 계정과 서버가 필요한 공유 기능은 자체 호스팅 서버로 제공한다. | 2–3 | [공유 일지 Windows 기능](https://dayoneapp.com/guides/shared-journals/shared-journals-features-by-platform/), [공유 일지](https://dayoneapp.com/guides/shared-journals/shared-journals/) |
| AI 보조 | 일일 대화, 글 하이라이트, 더 깊은 질문, 제목 제안, 여러 글 요약, 이미지 생성에 대응하는 기능을 유료 잠금 없이 제공한다. 기본 경로는 로컬 모델이며 지원 하드웨어·모델 다운로드 용량·처리 시간·품질을 공개한다. | 3 | [Gold 기능표](https://dayoneapp.com/guides/premium-subscription/day-one-pricing-features-guide/) |
| 외부 연동 | 이메일에서 글 가져오기, 자동화용 로컬 API/웹훅, 인쇄용 PDF를 제공한다. 문자 수신·발송과 실물 인쇄 주문은 외부 서비스 계약이 필요한 별도 연결 기능으로 정의한다. | 3 | [요금 및 기능표](https://dayoneapp.com/guides/premium-subscription/day-one-pricing-features-guide/) |

### 무료 원칙

- 회원 가입 없이 로컬의 모든 개인 일기 기능을 사용한다.
- 구독 화면, 광고, 기능 잠금, 글/일지/첨부 개수에 따른 과금을 넣지 않는다.
- 동기화와 AI도 앱이 과금하지 않는다. 외부 서비스 연결은 사용자가 명시적으로 선택하며 자체 호스팅과 로컬 대안을 제공한다.
- 저장 용량은 사용자 기기 또는 사용자가 선택한 저장소의 물리적 한도에 따른다. "무제한 클라우드 저장"은 약속하지 않는다.

## 4. 화면과 핵심 흐름

1. **첫 실행:** 로컬 저장 위치와 암호 잠금 선택 → 빈 일지 생성 또는 Day One ZIP 가져오기 → 타임라인 진입. 계정 생성은 선택 사항이다.
2. **기록:** 새 글 → 날짜/일지 선택 → 서식·태그·첨부·위치 추가 → 자동 저장 → 타임라인·달력에서 확인.
3. **찾기:** 검색어 및 태그/기간/첨부/즐겨찾기 필터 → 결과 열기 → 편집 또는 내보내기.
4. **회고:** 달력 또는 이 날의 기록 → 과거 글 보기 → 후속 글 작성.
5. **공유:** 공유 일지 생성 → 초대 링크 → 참여 승인 → 권한에 따른 글·댓글·반응 동기화.
6. **안전한 이전:** Day One JSON ZIP 선택 → 항목·첨부 수와 오류 미리보기 → 가져오기 → 원본 대비 건수·파일 해시 확인.

기본 화면은 **좌측 일지/설정, 중앙 글 목록, 우측 글 내용**의 데스크톱 레이아웃으로 제안한다. 타임라인·달력·미디어 전환, 검색, 새 글을 상단에 둔다. 이는 구현 제안이며 원본 화면의 픽셀 단위 복제를 요구하지 않는다.

## 5. 데이터와 기술 설계 초안

- **클라이언트:** Windows 데스크톱 앱. 설치·자동 업데이트·오프라인 실행을 지원한다. 구현 프레임워크는 프로토타입에서 성능·접근성·배포 크기를 비교해 결정한다.
- **로컬 데이터:** SQLite 메타데이터 DB와 콘텐츠 주소 기반 첨부 저장소. `Journal`, `Entry`, `EntryRevision`, `Attachment`, `Tag`, `Template`, `Reminder`, `ShareMembership`를 분리한다. 날짜는 원래 시간대와 UTC를 함께 보관한다.
- **보호:** 검증된 암호화 라이브러리로 DB/첨부/백업을 보호하고 키는 OS 보안 저장소와 사용자 복구 구문으로 관리한다. 암호화 방식과 키 복구 설계는 구현 전 별도 보안 설계 검토를 거친다.
- **동기화:** 로컬 변경 로그와 기기별 식별자로 재시도·중복 제거·충돌 표시를 구현한다. 서버에는 암호문만 전송한다. 공유 기능에는 초대·권한 관리 서버가 필요하다.
- **Google Drive 연동:** 개인 일지의 암호화 동기화와 사용자가 볼 수 있는 백업을 구분한다. 권한, 키 관리, 충돌 처리, 복구 흐름은 [Google Drive 연동 설계](GOOGLE_DRIVE_DESIGN.md)를 따른다.
- **상호 운용:** 내부 스키마를 Day One의 사유 형식에 종속시키지 않는다. Day One JSON ZIP은 버전별 변환기로 읽고, 미지원 필드는 손실 보고서에 표시한다.
- **AI:** 사용자가 명시적으로 켰을 때만 로컬 기록을 모델에 전달한다. 기본 설정에서는 외부 AI API로 일기를 전송하지 않는다.

## 6. 검증 순서와 출시 기준

| 단계 | 결과물 | 통과 조건 |
| --- | --- | --- |
| 조사 보강 | Windows 앱 시나리오별 화면/동작 기록, Day One JSON 샘플 스키마, 기능 대응표 | 앱 실행 환경을 복구한 뒤 새 글·검색·첨부·내보내기·잠금·공유를 테스트 데이터로 확인한다. 관찰 불가 항목은 미확인으로 남긴다. |
| 1차 구현 | 로컬 일지, 편집기, 검색/보기, 첨부, 잠금, Day One 가져오기·자체 내보내기 | 오프라인에서 전체 기능 동작. 1,000개 글/다양한 첨부의 가져오기·내보내기 왕복에서 본문·날짜·태그·첨부가 보존된다. 비정상 종료 뒤 마지막 저장 상태가 복구된다. |
| 2차 구현 | 알림·템플릿·암호화 백업·기기 동기화 | 오프라인 양방향 편집 후 누락·무음 덮어쓰기 없이 동기화. 키 없는 장치/서버에서 본문을 읽을 수 없다. |
| 3차 구현 | 공유 일지, 외부 연동, 로컬 AI | 초대·권한 회수·댓글·동시 편집 충돌을 검증한다. AI 기능별 지원 하드웨어에서 품질·속도를 측정한다. 앱 내 유료 잠금이 없다. |

## 7. 남은 결정과 제약

1. **완전한 기능 동일성:** Windows 앱의 실측 UI 검증이 아직 없다. 공식 문서가 설명하는 기능과 정적 채널은 직접 동작 확인의 대체물이 아니다.
2. **무료 동기화의 운영 주체:** 무제한 중앙 저장소를 무료로 계속 운영하는 모델은 이 기획만으로 성립하지 않는다. 초기 구현은 로컬 사용과 자체 호스팅 동기화를 기준으로 한다.
3. **AI 동등성:** 로컬 모델의 결과 품질과 처리 속도가 Day One Gold와 같다는 보장은 없다. 기능 형태를 맞춘 뒤 실제 품질을 따로 평가해야 한다.
4. **제삼자 기능:** SMS 수신, 상용 인쇄, Day One 서버와의 직접 동기화는 독립 앱이 임의로 동일하게 제공할 수 없다. 데이터 이전은 공식 JSON 내보내기·가져오기 경로를 사용한다.
5. **독립성:** 이름·아이콘·문구·원본 코드·사유 서버 프로토콜을 복제하지 않고, 관찰 가능한 기능 요구사항에 맞춰 새로 구현한다.

