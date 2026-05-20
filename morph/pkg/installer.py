import os
import tarfile
import urllib.request


class Installer:
    def __init__(self, project_root: str):
        self.pkg_dir = os.path.join(project_root, ".morph", "packages")

    def install(self, meta: dict) -> None:
        os.makedirs(self.pkg_dir, exist_ok=True)
        tarball_url = meta["tarball"]
        dest = os.path.join(self.pkg_dir, meta["name"])
        os.makedirs(dest, exist_ok=True)

        tmp = os.path.join(self.pkg_dir, "_tmp.tar.gz")
        urllib.request.urlretrieve(tarball_url, tmp)

        with tarfile.open(tmp) as tar:
            tar.extractall(dest)

        os.remove(tmp)
