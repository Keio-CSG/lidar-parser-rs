# from LSLIDAR_CH128X1_ROS驱动_V1.0.4_211026_ROS\lslidar_ch128x1\lslidar_ch128x1_decoder\include\lslidar_ch_decoder\lslidar_ch_decoder.h
import numpy as np
import matplotlib.pyplot as plt

big_angle = [
    -17, -16, -15, -14, -13, -12, -11, -10,
    -9, -8, -7, -6, -5, -4.125, -4, -3.125,
    -3, -2.125, -2, -1.125, -1, -0.125, 0, 0.875,
    1, 1.875, 2, 3, 4, 5, 6, 7
]

def sin_theta_one(ch):
    return np.sin(big_angle[ch // 4] * np.pi / 180)

def cos_theta_one(ch):
    return np.cos(big_angle[ch // 4] * np.pi / 180)

def sin_theta_two(ch):
    return np.sin((ch % 4) * -0.17 * np.pi / 180)

def cos_theta_two(ch):
    return np.cos((ch % 4) * -0.17 * np.pi / 180)

v_angle_list = np.zeros((128, 12))

for ch in range(128):
    for azimuth in np.arange(30, 150, 10):
        azimuth_rad = azimuth * np.pi / 180
        R = cos_theta_two(ch) * cos_theta_one(ch) * np.cos(azimuth_rad / 2) - sin_theta_two(ch) * sin_theta_one(ch)
        sin_theat = sin_theta_one(ch) + 2 * R * sin_theta_two(ch)

        v_angle = np.arcsin(sin_theat) * 180 / np.pi
        v_angle_list[ch, (azimuth - 30) // 10] = v_angle

print(v_angle_list)

plt.figure(figsize=(20,20))
for ch in range(128):
    plt.plot(v_angle_list[ch, :], 'o-')
plt.show()