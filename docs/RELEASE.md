# Cutting a CelestineMD release

## Versioning & in-app notices

CelestineMD uses `MAJOR.MINOR.PATCH` (e.g. `0.2.0`):

| Bump | Use for | In-app “update available” |
|------|---------|---------------------------|
| **MINOR** (or MAJOR) | Feature / bigger functionality | Yes — banner links to GitHub Releases |
| **PATCH** | Bugfixes / small improvements | No |

The app compares the running build to the latest GitHub release and notifies only when the feature line (`MAJOR.MINOR`) advances. Set `CELESTINE_NO_UPDATE=1` to disable the check.

## What this repo automates

GitHub Actions (`.github/workflows/release.yml`) builds **Linux**, **macOS** (Intel + Apple Silicon), and **Windows** archives when you push a version tag (`v0.1.0`, …).

## What you must do (accounts I can’t use for you)

| Channel | Ready in-repo? | Needs your action |
|---------|----------------|-------------------|
| **GitHub Releases** (binaries) | ✅ workflow | `gh auth login`, push tag |
| **Homebrew** | ✅ formula draft | Create tap `nek0pi/homebrew-celestine`, fill `sha256`, `brew tap` |
| **Arch AUR** | ✅ `PKGBUILD` | AUR account + push package |
| **Flathub / Snap** | ❌ not set up | Separate packaging + review queues |

I cannot publish to Homebrew core, AUR, or Flathub without your credentials and review processes.

---

## 1. First release checklist

```bash
# From a clean tree with release-ready code committed & pushed to main:
git tag -a v0.1.0 -m "CelestineMD v0.1.0"
git push origin main
git push origin v0.1.0
```

Watch **Actions → Release**. When green, the GitHub Release page will have:

- `celestine-linux-x86_64-v0.1.0.tar.gz`
- `celestine-macos-aarch64-v0.1.0.tar.gz`
- `celestine-macos-x86_64-v0.1.0.tar.gz`
- `celestine-windows-x86_64-v0.1.0.zip`

### Local Linux tarball (optional, without CI)

```bash
./packaging/build-linux-dist.sh 0.1.0
# → dist/celestine-linux-x86_64-v0.1.0.tar.gz
./dist/celestine-linux-x86_64/install.sh   # installs to ~/.local
```

---

## 2. Homebrew (your tap)

1. Create a public repo: `https://github.com/nek0pi/homebrew-celestine`
2. Copy `packaging/homebrew/celestine-md.rb` → `Formula/celestine-md.rb`
3. After the GitHub tag exists:

```bash
curl -sL https://github.com/nek0pi/celestine-md/archive/refs/tags/v0.1.0.tar.gz | shasum -a 256
# paste into sha256 "..." in the formula
```

4. Users install:

```bash
brew tap nek0pi/celestine
brew install celestine-md
# or until the first tag SHA is set:
brew install --HEAD nek0pi/celestine/celestine-md
```

Getting into **homebrew-core** is a separate PR review and usually wants notable usage — start with your tap.

---

## 3. Arch Linux (AUR)

1. Register on [aur.archlinux.org](https://aur.archlinux.org/)
2. `git clone ssh://aur@aur.archlinux.org/celestine-md.git`
3. Copy `packaging/aur/PKGBUILD`, set real `sha256sums`, run:

```bash
makepkg --printsrcinfo > .SRCINFO
makepkg -si   # test locally
git add PKGBUILD .SRCINFO && git commit -m "celestine-md 0.1.0-1" && git push
```

Users: `yay -S celestine-md` / `paru -S celestine-md`.

Optional later: `celestine-md-bin` PKGBUILD that downloads the GitHub release binary (faster installs).

---

## 4. End-user install summary

| OS | Easiest path |
|----|----------------|
| **Linux** | Download `.tar.gz` → `./install.sh`, or AUR when published |
| **macOS** | Download arch-matching `.tar.gz`, or `brew tap` when published |
| **Windows** | Download `.zip`, run `celestine.exe` (SmartScreen may warn on first unsigned builds) |

Code-sign / notarize for macOS and Authenticode for Windows are **not** included yet (needs Apple/Microsoft developer accounts + certificates).
