# ALUS (Atomic Linux Update System)

## How To

### Users

You will likely only need to over run `alus update`, for any other needs, just use `alus --help` and explore!

### Distributors

1. Build ALUS using the cargo toolchain via `cargo build --release --bins`.
2. Move `./target/release/alus` to `/usr/bin/alus`
3. Move `./target/release/alus-initramfs` to `/bin/alus-initramfs` inside the initramfs. The details of how may change depending on what initramfs generator you use (Dracut, Booster, Mkinitcpio, etc) and are thus not detailed here.
4. The kernel commandline argument `commit_hash` should be specified and pointing to the commit hash generated. We recommend setting this via hooks (explained in the next step)
5. Setup hooks. We have a large portion precreated [here.](https://github.com/taxrosdev/alus-hooks)
If you wish to write your own, there is [documentation.](https://github.com/alus#hooks)
6. Setup a step in your build pipeline to run `alus --usr <USR_PATH> commit <INITRAMFS_PATH> <VMLINUZ_PATH>` to commit to the now-created Repository. Make sure to save the commit hash returned!
7. Host `/.alus` whilst ignoring permissions of the Repository.

### Installers

> These next steps assume `/mnt` is the install location, and `https://example.com/repo` is the network location where your Repository is hosted.
1. Include the `alus` command created by the Distributors. You don't need to include `alus-initramfs`.
2. On install, `alus --repo /mnt config remote https://example.com/repo`
> If you wish to make a non-network installer or even just use the installer medium for speed (Recommended), just add --clone-from /.alus (NOT /mnt/.alus) to include the installers blobs in the next command.
> If you wish to additionally make the installer non-network and disable updates, you need to copy `/.alus/branch/<BRANCH>` to `/mnt/.alus/branch/<BRANCH>`.
3. After the above configuration steps, you can now actually start the installation process with `alus switch <BRANCH>`

## Architecture

### Hooks

Actions may need to be taken during updates to certain files, both within usr, etc, and other misc paths.
These actions are called Hooks and are triggered right before the usr tree is swapped, and can be used to modify usr, etc, and other misc paths.

## Credits

### Inspiration

This project is heavily inspired by systemds immutable tools, and AerynOS.
Both projects mentioned above are completely fine and working, they may fit your usecase better than ALUS, we simply had a different usecase.

## No AI/LLM Generated Commits

Unfortunately we cannot assert their quality, or the reviewers quality, and quite frankly theres a large portion of discourse surrounding them that we do not want to associate with.

If you can author code with a LLM and we don't notice, you're probably fine using AI within the context of this project.
