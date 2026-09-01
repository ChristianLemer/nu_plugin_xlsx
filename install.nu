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
    let assets = (
      http get $"https://api.github.com/repos/($repo)/releases"
      | each {|r| $r.assets | each {|a| {tag: $r.tag_name, name: $a.name, url: $a.browser_download_url}}}
      | flatten
    )
    let esc = ($minor | str replace --all "." '\.')
    let hit = ($targets | each {|t|
        $assets | where name =~ ('nu' + $esc + '\.[0-9]+-' + $t + '\.(tar\.gz|zip)$')
      } | flatten)

    if ($hit | is-empty) {
      let have = ($assets | get name | parse --regex 'nu(?<v>\d+\.\d+)\.' | get v | uniq | sort)
      error make {
        msg: (if ($have | is-empty) {
          $"No archives published under this naming scheme in ($repo)."
        } else {
          $"No build for Nushell ($minor). Available: ($have | str join ', ')."
        })
        help: "Upgrade Nushell, or build from the tag matching your version."
      }
    }

    let asset = ($hit | last)
    print $"Found  ($asset.name)  [($asset.tag)]"
    if $dry_run { print $"(dry-run) would install ($bin)"; return {asset: $asset.name, dest: $bin} }

    let tmp = ($nu.temp-dir | path join $asset.name)
    http get $asset.url | save --force --raw $tmp
    let sum = ($assets | where name == $"($asset.name).sha256")
    if not ($sum | is-empty) {
      let want = (http get ($sum | first | get url) | str trim | split row " " | first)
      if $want != (open --raw $tmp | hash sha256) {
        error make {msg: $"Checksum mismatch for ($asset.name) — refusing to install."}
      }
      print "Checksum verified"
    }
    $tmp
  }

  if $dry_run { print $"(dry-run) would install ($bin)"; return {dest: $bin} }

  mkdir $dest
  # `tar -xf` detects gzip and zip alike, including the bsdtar shipped with
  # Windows 10+. One extraction path, no per-platform branch.
  ^tar -xf $file -C $dest
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
