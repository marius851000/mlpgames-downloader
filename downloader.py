from typing import Dict, Any
import os
import requests
import subprocess
import json
import argparse

class Config:
    def __init__(self, dest_folder: str, base_url: str = "https://games.mare.by/", base_api_url: str = "https://games.mare.by/"):
        self.dest_folder = dest_folder
        self.base_url = base_url
        self.base_api_url = base_api_url

    def get_tmp_folder(self):
        return os.path.join(self.dest_folder, "tmp")

    def get_meta_path(self, id: int) -> str:
        return os.path.join(self.dest_folder, str(id) + ".meta.json")

    def get_file_path(self, id: int) -> str:
        return os.path.join(self.dest_folder, str(id))

    def get_source_url(self, id: int) -> str:
        return self.base_url + str(id)

class StateManager:
    def __init__(self, config: Config):
        self.config: Config = config
        os.makedirs(config.get_tmp_folder(), exist_ok=True)
        req = requests.get(self.config.base_api_url + "files")
        self.downloaded_data = req.json()

    def get_entry_by_id(self, id: int) -> Dict[str, Any]:
        for i in self.downloaded_data:
            if int(i["id"]) == id:
                return i
        raise BaseException("no game with id " + str(id))


    def download_entry(self, id: int):
        entry = self.get_entry_by_id(id)
        if os.path.exists(self.config.get_meta_path(id)):

            with open(self.config.get_meta_path(id)) as f:
                on_disk_meta = json.load(f)
            server_info = entry["files"][0]
            if on_disk_meta["modified"] == server_info["modified"] and on_disk_meta["path"] == server_info["path"] and on_disk_meta["size"] == server_info["size"]:
                return
            else:
                print("local and online meta differs, re-downloading")
                print(on_disk_meta)
                print(server_info)

        print("downloading " + self.config.get_file_path(id) + " (" + entry["primary_file"] + ")")

        source_url = self.config.get_source_url(id)
        out_path = self.config.get_file_path(id)

        tmp_path = os.path.join(self.config.get_tmp_folder(), str(id))

        subprocess.check_call(["wget", source_url, "-O", tmp_path])
        downloaded_file_size = os.path.getsize(tmp_path)
        if downloaded_file_size != entry["files"][0]["size"]:
            raise BaseException("Downloaded files does not have the expected size " + str(entry["files"][0]["size"]))
        os.rename(tmp_path, out_path)

        assert len(entry["files"]) == 1

        tmp_path_json = os.path.join(self.config.get_tmp_folder(), str(id) + ".meta.json")
        with open(tmp_path_json, "w") as meta_file:
            json.dump(entry["files"][0], meta_file)
        os.rename(tmp_path_json, self.config.get_meta_path(id))

    def download_all(self):
        for i in self.downloaded_data:
            try:
                self.download_entry(int(i["id"]))
            except Exception as e:
                print(e)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="MLP Games Mirror Downloader")
    parser.add_argument("dest_folder", default="./dest", help="Destination folder for downloaded files")
    parser.add_argument("--base-url", default="https://games.mare.by/", help="Base URL for game downloads")
    parser.add_argument("--base-api-url", default="https://games.mare.by/", help="Base URL for API calls")
    args = parser.parse_args()

    c = Config(args.dest_folder, args.base_url, args.base_api_url)
    s = StateManager(c)
    s.download_all()
