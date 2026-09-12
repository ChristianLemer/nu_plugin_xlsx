# Install nu_plugin_xlsx matched to the Nushell running this script.
#
# A plugin binary loads into exactly one Nushell minor — the protocol version is a
# compile-time constant under a caret match. No package manager can express that
# coupling: the `+nu-0.115.1` build metadata is ignored by cargo for resolution.
# So this script resolves the selection itself, from the version executing it.
#
#   http get https://raw.githubusercontent.com/ChristianLemer/nu_plugin_xlsx/HEAD/install.nu | save -f install.nu
#   nu install.nu --register
#
# Re-run it after every Nushell upgrade — that is when plugins silently stop loading.
def main [
  --repo: string = "ChristianLemer/nu_plugin_xlsx"
  --dir: path        # where to put the binary (default $nu.data-dir/plugins)
  --archive: path    # install this local archive instead of downloading
  --register         # run `plugin add` once installed
  --dry-run          # show what would happen, write nothing
] {
  let os   = $nu.os-info
  let key  = $"($os.name)-($os.arch)"
  let dest = ($dir | default ($nu.data-dir | path join "plugins"))
  let exe  = if $os.name == "windows" { "nu_plugin_xlsx.exe" } else { "nu_plugin_xlsx" }
  let bin  = ($dest | path join $exe)

  let file = if $archive != null {
    print $"Nushell ((version).version) · ($key) · local archive"
    $archive
  } else {
    # Preference order, not a single value: CI falls back to a gnu build if the
    # musl link ever fails, and a musl-only client would then find nothing.
    let targets = match $key {
      "linux-x86_64"   => ["x86_64-unknown-linux-musl", "x86_64-unknown-linux-gnu"]
      "linux-aarch64"  => ["aarch64-unknown-linux-musl", "aarch64-unknown-linux-gnu"]
      "macos-aarch64"  => ["aarch64-apple-darwin"]
      "macos-x86_64"   => ["x86_64-apple-darwin"]
      "windows-x86_64" => ["x86_64-pc-windows-msvc"]
      _                => []
    }
    if ($targets | is-empty) {
      error make {msg: $"Unsupported platform: ($key). Build from source."}
    }
    let minor = ((version).version | split row "." | first 2 | str join ".")
    print $"Nushell ((version).version) · ($key) → ($targets | first)"

    # One release per Nushell minor, so the newest release is almost never the
    # right one. Every release has to be searched, not just `latest`.
    # Unauthenticated, the API allows 60 calls an hour per address — shared
    # runners and offices behind one NAT run out. A token lifts that. Only the
    # API call carries it: the download URLs redirect to signed storage, which
    # rejects a request that arrives with an Authorization header.
    let auth = if ($env.GITHUB_TOKEN? | default "" | is-empty) { [] } else {
      [Authorization $"Bearer ($env.GITHUB_TOKEN)"]
    }
    let assets = (
      http get --headers $auth $"https://api.github.com/repos/($repo)/releases?per_page=100"
      | each {|r| $r.assets | each {|a| {tag: $r.tag_name, name: $a.name, url: $a.browser_download_url}}}
      | flatten
    )
    let esc = ($minor | str replace --all "." '\.')
    let hit = ($targets | each {|t|
        $assets | where name =~ ('nu' + $esc + '\.[0-9]+-' + $t + '\.(tar\.gz|zip)$')
      } | flatten)

    if ($hit | is-empty) {
      # Two different failures, and they need different answers. The
      # architecture may have no build at all, or only this Nushell minor may
      # be missing for it. Telling someone on an unbuilt architecture to
      # upgrade Nushell sends them to fix what is not broken.
      let for_platform = ($targets | each {|t|
          $assets | where name =~ ('-' + $t + '\.(tar\.gz|zip)$')
        } | flatten)
      if ($for_platform | is-empty) {
        let built = ($assets | get name
          | parse --regex '-nu[0-9.]+-(?<t>.+)\.(?:tar\.gz|zip)$' | get t | uniq | sort)
        error make {
          msg: (if ($built | is-empty) {
            $"No archives published under this naming scheme in ($repo)."
          } else {
            $"No build for ($key). Built: ($built | str join ', ')."
          })
          help: "Build from source: check out the tag whose +nu- matches your Nushell, then `cargo install --path .`."
        }
      }
      let have = ($for_platform | get name | parse --regex 'nu(?<v>\d+\.\d+)\.' | get v | uniq | sort)
      error make {
        msg: $"No build for Nushell ($minor) on ($key). Available: ($have | str join ', ')."
        help: "Upgrade or downgrade Nushell to one of those, or build from the tag matching your version."
      }
    }

    # $hit is ordered by target preference, then newest release first, as the
    # API returns them — so the first match is the right one on both axes.
    let asset = ($hit | first)
    print $"Found  ($asset.name)  [($asset.tag)]"
    if $dry_run { print $"\(dry-run) would install ($bin)"; return {asset: $asset.name, dest: $bin} }

    let tmp = ($nu.temp-dir | path join $asset.name)
    http get $asset.url | save --force --raw $tmp
    let sum = ($assets | where name == $"($asset.name).sha256")
    if not ($sum | is-empty) {
      # GitHub serves the .sha256 as binary; decode before treating it as text.
      let want = (http get ($sum | first | get url) | decode utf-8 | str trim | split row " " | first)
      if $want != (open --raw $tmp | hash sha256) {
        error make {msg: $"Checksum mismatch for ($asset.name) — refusing to install."}
      }
      print "Checksum verified"
    }
    $tmp
  }

  if $dry_run { print $"\(dry-run) would install ($bin)"; return {dest: $bin} }

  mkdir $dest
  # `tar -xf` detects gzip and zip alike, including the bsdtar shipped with
  # Windows 10+ — which has to be named by full path: under Git Bash or MSYS a
  # GNU tar shadows it on PATH, and GNU tar cannot read a zip.
  let tar = if $os.name == "windows" { $env.SystemRoot | path join System32 tar.exe } else { "tar" }
  # The archive also carries LICENSE and README.md. They belong to the download,
  # not to a plugins directory shared with every other plugin, so the extraction
  # is staged and only the binary moves. Extracting one member by name would be
  # shorter and is not portable: the GNU tar on many Linux boxes cannot read a
  # zip at all, let alone select from one.
  # Unique per run. A fixed name under a shared temp directory is another
  # user's directory on a multi-user box, where removing it fails under the
  # sticky bit, and it is a race between two runs of this script.
  let stage = ($nu.temp-dir | path join $"nu_plugin_xlsx-(random chars --length 8)")
  mkdir $stage
  ^$tar -xf $file -C $stage
  mv ($stage | path join $exe) $bin
  rm --recursive --force $stage
  if $archive == null { rm --force $file }

  # Keep a copy beside the binary: re-running after a Nushell upgrade is then a
  # local command, with no URL to find again.
  cp ($env.CURRENT_FILE? | default "install.nu") ($dest | path join "install.nu")

  print $"Installed ($bin)"
  if $register {
    plugin add $bin
    print "Registered. Add `plugin use xlsx` to your config so it survives a restart."
  } else {
    print $"To enable it:\n  plugin add ($bin)\n  plugin use xlsx"
  }
  {dest: $bin}
}
