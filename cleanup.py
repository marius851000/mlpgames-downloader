import os
import json

BASE_DIR = "./dest"

for file_name in os.listdir(BASE_DIR):
    if file_name == "tmp":
        continue
    elif file_name.endswith(".meta.json"):
        base_file_path = os.path.join(BASE_DIR, file_name.split(".")[0])
        if not os.path.exists(base_file_path):
            print("base file for " + file_name + " does not exist")
        continue
    else:
        file_path = os.path.join(BASE_DIR, file_name)
        meta_path = os.path.join(BASE_DIR, file_name + ".meta.json")
        if not os.path.exists(meta_path):
            print(".meta.json does not exist for " + file_name)
            continue

        with open(meta_path, "r") as f:
            meta_data = json.load(f)

        size = os.path.getsize(file_path)
        if size != meta_data["size"]:
            print("file " + file_name + " should be " + str(meta_data["size"]) + "B but is " + str(size) + "B")
