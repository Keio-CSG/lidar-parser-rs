# AT128の角度補正データ(バイナリ)をテキストファイルに変換するスクリプト
# 詳細はAT128のマニュアルを参照のこと
# 抽出した定数はconstants_at128.rsに記述している

import struct

with open("default_AT.dat", "rb") as f:
    data = f.read()
    
    channel_num = data[4]      # N
    mirror_num = data[5]       # M
    frame_num = data[6]        # F
    resolution_deg = data[15]  # resolution
    cursor = 16
    start_frame = []
    for i in range(mirror_num):
        start_frame.append(struct.unpack_from("<I", data, cursor)[0])
        cursor += 4
    end_frame = []
    for i in range(mirror_num):
        end_frame.append(struct.unpack_from("<I", data, cursor)[0])
        cursor += 4

    azimuth_offset = []
    for i in range(channel_num):
        azimuth_offset.append(struct.unpack_from("<i", data, cursor)[0])
        cursor += 4
    elevation_offset = []
    for i in range(channel_num):
        elevation_offset.append(struct.unpack_from("<i", data, cursor)[0])
        cursor += 4
    
    azimuth_adjust = []
    for row in range(128):
        azimuth_adjust.append([])
        for column in range(180):
            azimuth_adjust[row].append(data[cursor])
            cursor += 1

    elevation_adjust = []
    for row in range(128):
        elevation_adjust.append([])
        for column in range(180):
            elevation_adjust[row].append(data[cursor])
            cursor += 1

    with open("at128_angle_correction.txt", "w") as f:
        f.write("channel_num: " + str(channel_num) + "\n")
        f.write("mirror_num: " + str(mirror_num) + "\n")
        f.write("frame_num: " + str(frame_num) + "\n")
        f.write("resolution_deg: " + str(resolution_deg) + "\n")
        f.write("start_frame: " + str(start_frame) + "\n")
        f.write("end_frame: " + str(end_frame) + "\n")
        f.write("azimuth_offset: " + str(azimuth_offset) + "\n")
        f.write("elevation_offset: " + str(elevation_offset) + "\n")
        f.write("azimuth_adjust: " + str(azimuth_adjust) + "\n")
        f.write("elevation_adjust: " + str(elevation_adjust) + "\n")
