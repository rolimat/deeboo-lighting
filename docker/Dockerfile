FROM rust:1.99.1-bookworm

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

ARG NODE_VERSION=26.13.0
ARG JUST_VERSION=1.59.0
ARG CARGO_DENY_VERSION=0.20.2
ARG LEFTHOOK_VERSION=2.2.2
ARG HADOLINT_VERSION=2.15.1
ARG GITLEAKS_VERSION=8.30.1

# SHA-256 of every downloaded file, from each project's release checksum file.
ARG NODE_SHA256_AMD64=5848553868b402d3fabb5b09270649977780a390f832155721025a08b5104455
ARG NODE_SHA256_ARM64=439e9c438acdf6a6ffe47418da709153d66c4bed812e30a473faacb107dd2791
ARG JUST_SHA256_AMD64=409044617b6f06517c1035e55ca38c8c856ff8b650e41bc2aa1ef82d5903a23e
ARG JUST_SHA256_ARM64=23871b1702bfd550fb5f901778fd4290ecf2ee034a45a889fbe96d89b188086d
ARG CARGO_DENY_SHA256_AMD64=9f12ed4c49936e09b48bf862b595cde2fe64fcbd9d74dfacac6131ca824c8d5f
ARG CARGO_DENY_SHA256_ARM64=995c82be0defc7a025cae49a2aa2644ce8245c9a3318fc4103907c6a285e8c7d
ARG LEFTHOOK_SHA256_AMD64=4fbf89eb7748c6aed84d833ee1a29edac48fae614468ca9a82896ec32f2800b6
ARG LEFTHOOK_SHA256_ARM64=a4d4bf78d902af1a148bf0662af08e13b791061e46e6e2183b0660eed96b43f6
ARG HADOLINT_SHA256_AMD64=c7187db94eeeeca956519a6af171adc31453941a1e777961f6e680f697c8c507
ARG HADOLINT_SHA256_ARM64=f6198ef8090f404dbb771abfee086eb8c48ac177f30da7fd3510aca35b344b5d
ARG GITLEAKS_SHA256_AMD64=551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb
ARG GITLEAKS_SHA256_ARM64=e4a487ee7ccd7d3a7f7ec08657610aa3606637dab924210b3aee62570fb4b080

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
    build-essential pkg-config libssl-dev libwebkit2gtk-4.1-dev libxdo-dev \
    libayatana-appindicator3-dev librsvg2-dev libasound2-dev curl ca-certificates git \
 && rm -rf /var/lib/apt/lists/*

WORKDIR /tmp

# Pinned binaries, picked per CPU architecture (arm64 locally, amd64 on CI), each verified by SHA-256.
RUN fetch() { curl -fsSL -o "$1" "$2" && echo "$3  $1" | sha256sum -c -; } \
 && case "$(dpkg --print-architecture)" in \
      amd64) musl=x86_64-unknown-linux-musl; lh=x86_64; hl=x86_64; gl=x64; node=x64; \
             n=$NODE_SHA256_AMD64; j=$JUST_SHA256_AMD64; d=$CARGO_DENY_SHA256_AMD64; l=$LEFTHOOK_SHA256_AMD64; h=$HADOLINT_SHA256_AMD64; g=$GITLEAKS_SHA256_AMD64 ;; \
      arm64) musl=aarch64-unknown-linux-musl; lh=aarch64; hl=arm64; gl=arm64; node=arm64; \
             n=$NODE_SHA256_ARM64; j=$JUST_SHA256_ARM64; d=$CARGO_DENY_SHA256_ARM64; l=$LEFTHOOK_SHA256_ARM64; h=$HADOLINT_SHA256_ARM64; g=$GITLEAKS_SHA256_ARM64 ;; \
      *) echo "unsupported architecture" >&2; exit 1 ;; \
    esac \
 && fetch node.tgz "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-${node}.tar.gz" "$n" \
 && tar -xz --strip-components=1 -C /usr/local -f node.tgz --exclude='*.md' --exclude=LICENSE \
 && fetch just.tgz "https://github.com/casey/just/releases/download/${JUST_VERSION}/just-${JUST_VERSION}-${musl}.tar.gz" "$j" \
 && tar -xz -C /usr/local/bin -f just.tgz just \
 && fetch deny.tgz "https://github.com/EmbarkStudios/cargo-deny/releases/download/${CARGO_DENY_VERSION}/cargo-deny-${CARGO_DENY_VERSION}-${musl}.tar.gz" "$d" \
 && tar -xz --strip-components=1 -C /usr/local/bin -f deny.tgz "cargo-deny-${CARGO_DENY_VERSION}-${musl}/cargo-deny" \
 && fetch /usr/local/bin/lefthook "https://github.com/evilmartians/lefthook/releases/download/v${LEFTHOOK_VERSION}/lefthook_${LEFTHOOK_VERSION}_Linux_${lh}" "$l" \
 && fetch /usr/local/bin/hadolint "https://github.com/hadolint/hadolint/releases/download/v${HADOLINT_VERSION}/hadolint-linux-${hl}" "$h" \
 && fetch gitleaks.tgz "https://github.com/gitleaks/gitleaks/releases/download/v${GITLEAKS_VERSION}/gitleaks_${GITLEAKS_VERSION}_linux_${gl}.tar.gz" "$g" \
 && tar -xz -C /usr/local/bin -f gitleaks.tgz gitleaks \
 && chmod +x /usr/local/bin/lefthook /usr/local/bin/hadolint \
 && rm -f /tmp/*.tgz

RUN rustup component add rustfmt clippy

ENV DEEBOO_IN_CONTAINER=1
WORKDIR /work
