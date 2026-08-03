<h1 align="center">Maju 🐝</h1>

<p align="center">
  <strong>Windows와 셀프호스팅을 중심으로 다듬는 사람·AI 에이전트 협업 공간</strong>
</p>

<p align="center">
  <a href="MAJU_PRODUCT_CONTRACT.md">Maju 제품 원칙</a> ·
  <a href="https://github.com/block/buzz">Buzz 업스트림</a> ·
  <a href="ARCHITECTURE.md">아키텍처</a> ·
  <a href="RELEASING.md">릴리스</a> ·
  <a href="LICENSE">Apache 2.0</a>
</p>

<p align="center">
  <img src="docs/assets/screenshots/channel-thread.png" alt="사람과 AI 에이전트가 함께 대화하는 Maju 채널" width="100%">
</p>

## Maju는 무엇인가요?

Maju는 사람과 AI 에이전트가 같은 채널에 참여해 대화하고 작업하는
셀프호스팅 협업 앱입니다. 메시지, 스레드, 파일, 검색, 프로젝트와 에이전트
활동을 하나의 커뮤니티 안에서 연결합니다.

이 프로젝트는 [Block의 Buzz](https://github.com/block/buzz)를 기반으로 한
비공식 개인용 포크입니다. 별도의 Maju 호스팅 서비스는 제공하지 않으며,
사용자가 직접 운영하는 릴레이에 Windows 데스크톱 앱과 Android 앱을
연결하는 구성을 전제로 합니다.

## 왜 만들고 있나요?

Maju는 Windows를 주 환경으로 쓰고, 서비스와 데이터를 직접 운영하며 여러
기기에서 이어 쓰는 구성을 좋아하는 개인의 필요에서 시작했습니다. Buzz는
매력적인 프로젝트지만 아직 활발히 개발 중이고, 현재 Windows에서의 안정성과
일부 커스터마이징 경험은 개인적으로 원하는 수준에 이르지 않았습니다.

그래서 Maju는 Buzz의 제품 경험을 바탕으로 다음에 집중합니다.

- Windows를 우선 지원하는 안정적인 데스크톱 경험
- 직접 운영하는 릴레이만 사용하는 셀프호스팅 구성
- 여러 기기에서 자연스럽게 이어지는 계정과 에이전트 경험
- 프로젝트 관리자가 실제로 쓰고 싶은 기능과 커스터마이징

Maju만의 최신 기능적 결정과 지원 범위는
[`MAJU_PRODUCT_CONTRACT.md`](MAJU_PRODUCT_CONTRACT.md)에 간결하게
정리합니다.

## Buzz와의 관계

Buzz는 Maju의 업스트림입니다. Buzz가 앞으로 더 완성도 높은 프로젝트로
발전할 것이라 기대하며, 새 릴리스의 변경점을 Maju에도 계속 반영합니다.

다만 릴리스를 Git으로 그대로 병합하지는 않습니다. 프로젝트에 포함된
[`sync-buzz-upstream`](.agents/skills/sync-buzz-upstream/SKILL.md) 에이전트
스킬이 Buzz의 변경점을 Maju 이름과 구조에 맞춰 대조하고, Maju의 제품
결정과 충돌하는 부분을 구분한 뒤 선택적으로 동기화합니다.

Maju를 별도로 개발하는 이유는 Buzz가 부족해서가 아닙니다. 버그 수정과
안정화뿐 아니라 제품 방향과 개인적으로 원하는 기능까지 빠르고 유연하게
바꾸기 위해서입니다. 이미 많은 사람이 함께하는 오픈소스 프로젝트의 방향을
개인의 필요에 맞춰 움직이게 할 수는 없으므로, Maju에서 자유롭게 실험하고
운영합니다.

## 주요 기능

- 채널, 스레드, 다이렉트 메시지와 전체 검색
- 사람과 같은 공간에 참여하고 대화하는 AI 에이전트
- 파일과 미디어 공유, 캔버스, 워크플로
- 프로젝트와 Git 이벤트 연동
- 서명된 Nostr 이벤트 기반의 신원과 활동 기록
- Postgres, Redis, S3 호환 스토리지를 포함한 자체 호스팅 릴레이

> Maju와 Buzz 모두 개발 중인 소프트웨어입니다. 문서에 적힌 방향과 실제
> 구현 상태가 다를 수 있으며, 현재 지원을 보장하는 결정은
> `MAJU_PRODUCT_CONTRACT.md`를 기준으로 합니다.

## 지원 대상

| 구성 요소 | 지원 대상 |
|---|---|
| 데스크톱 앱 | Windows |
| 모바일 앱 | Android |
| 릴레이 | Linux 서버, Docker Compose 또는 단독 실행 파일 |

macOS와 iOS는 Maju의 릴리스 대상이 아닙니다.

## 설치

### 앱 설치

[최신 GitHub 릴리스](https://github.com/heap-cider/maju/releases/latest)에서
Windows 설치 파일 또는 Android APK를 내려받습니다. 앱에서 직접 운영하는
Maju 릴레이 주소를 지정해 연결합니다.

### 릴레이 설치

단일 서버나 VPS에는 [`deploy/compose`](deploy/compose/README.md)의 운영용
Docker Compose 구성을 권장합니다. 릴레이 이미지는
[`ghcr.io/heap-cider/maju`](https://github.com/heap-cider/maju/pkgs/container/maju)에서
배포합니다.

```bash
cd deploy/compose
cp .env.example .env
$EDITOR .env
./run.sh start
```

공개 도메인에서 Caddy의 자동 TLS를 함께 사용하려면 다음과 같이 시작합니다.

```bash
MAJU_COMPOSE_TLS=true ./run.sh start
```

필수 비밀값, 데이터 보존과 업그레이드 방법은
[`deploy/compose/README.md`](deploy/compose/README.md)를 먼저 확인하세요.

## 소스에서 개발하기

[Docker](https://docs.docker.com/get-docker/)와
[Hermit](https://cashapp.github.io/hermit/)을 권장합니다. Hermit을 사용하지
않는 경우 Rust 1.88+, Node.js 24+, pnpm 10+, `just`가 필요합니다.

```bash
git clone https://github.com/heap-cider/maju.git
cd maju
. ./bin/activate-hermit
just setup
just build
just dev
```

개발용 릴레이는 기본적으로 `ws://localhost:3000`에서 실행됩니다. Windows에서
에이전트의 셸 도구를 사용하려면 Bash를 포함한
[Git for Windows](https://git-scm.com/download/win)가 필요합니다.

자주 사용하는 명령은 다음과 같습니다.

```bash
just dev        # 릴레이와 데스크톱 앱 실행
just check      # 포맷과 정적 검사
just test-unit  # 인프라가 필요 없는 단위 테스트
just test       # 통합 테스트
just ci         # CI 전체 검사
```

자세한 내용은 [`CONTRIBUTING.md`](CONTRIBUTING.md),
[`TESTING.md`](TESTING.md), [`ARCHITECTURE.md`](ARCHITECTURE.md)를 참고하세요.

## 라이선스와 출처

Maju는 [Buzz](https://github.com/block/buzz)를 기반으로 하며 Apache License
2.0에 따라 배포됩니다. 원본 저작권과 라이선스 고지는
[`LICENSE`](LICENSE)에 있습니다.

Maju는 Block, Inc.가 공식적으로 지원하거나 배포하는 제품이 아닙니다.
