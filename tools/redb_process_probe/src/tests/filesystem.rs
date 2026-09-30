use std::path::Path;

use super::*;

const SAMPLE: &str = "\
22 1 0:21 / / rw,relatime shared:1 - btrfs /dev/nvme0n1p3 rw,subvol=/root
35 22 0:31 / /tmp rw,nosuid,nodev shared:17 - tmpfs tmpfs rw,size=16G
61 22 259:1 / /run/media/system/Disk_2 rw,relatime shared:30 - ext4 /dev/nvme1n1p1 rw
62 22 259:2 / /run/media/system/My\\040Disk rw,relatime shared:31 - xfs /dev/sda1 rw
63 61 0:40 / /run/media/system/Disk_2/overlay rw - overlay overlay rw,lowerdir=/a
not a mount line
";

#[test]
fn mountinfo_lines_parse_with_escapes() {
    let mounts = parse_mountinfo(SAMPLE);
    assert_eq!(mounts.len(), 5);
    assert_eq!(
        mounts[3],
        Mount {
            mount_point: "/run/media/system/My Disk".to_string(),
            fs_type: "xfs".to_string(),
            source: "/dev/sda1".to_string(),
        }
    );
}

#[test]
fn the_longest_whole_component_mount_wins() {
    let mounts = parse_mountinfo(SAMPLE);
    let fs_type = |path: &str| {
        mount_of(Path::new(path), &mounts)
            .map(|mount| mount.fs_type.clone())
            .unwrap_or_default()
    };
    assert_eq!(fs_type("/run/media/system/Disk_2/Projects/x.redb"), "ext4");
    assert_eq!(fs_type("/run/media/system/Disk_2/overlay/db"), "overlay");
    assert_eq!(fs_type("/run/media/system/Disk_20/db"), "btrfs");
    assert_eq!(fs_type("/tmp/redb-probe"), "tmpfs");
    assert_eq!(fs_type("/run/media/system/My Disk/db"), "xfs");
}

#[test]
fn the_top_mount_on_a_shared_point_wins() {
    let text = "1 0 0:1 / /data rw - ext4 /dev/a rw\n2 1 0:2 / /data rw - tmpfs tmpfs rw\n";
    let mounts = parse_mountinfo(text);
    let mount = mount_of(Path::new("/data/x"), &mounts).expect("a mount matches");
    assert_eq!(mount.fs_type, "tmpfs");
}
