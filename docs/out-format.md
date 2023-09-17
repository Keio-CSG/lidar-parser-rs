# 出力フォーマット仕様 v1.0

## 変更履歴

- v1.0: 2023/09/17

## 概要

本リポジトリではLiDARのpcapファイルを2つの表形式ファイル(CSV, HDF5)に変換する。

各列はVeloViewの出力を参考に一部改変した形式とした。

```rust
pub struct VeloPoint {
    pub intensity: u8,   // calibrated reflectivity. values: 0-255
    pub channel: u8,     // a.k.a. laser id
    pub timestamp: u64,  // firing time. units: nanoseconds
    pub azimuth: u16,    // horizontal angle. units: 0.01 degrees
    pub altitude: i16,   // vertical angle. units: 0.01 degrees
    pub distance_m: f32, // distance. units: meters
    pub x: f32,          // cartesian coordinates (right-handed coordinate system)
    pub y: f32,          // units: meters
    pub z: f32,          //
}
```

## CSV出力

CSV出力では、フレームごとに個別のcsvファイルとして出力される。
ファイル名は元ファイルから以下のように決定される。

```
元ファイル: [filename].pcap
->
[filename]/[filename]_[frame].csv
([frame]はフレーム位置を5桁で表現)
```

例えばフレーム数100の`hoge.pcap`を入力した場合、出力ファイルは`hoge/hoge_00000.csv`から`hoge/hoge_00099.csv`の100個になる。

## HDF5出力

HDF5形式はThe HDF Groupによって策定されているファイルフォーマットで、階層的な表データを格納することができる。拡張子は.h5である。

https://www.hdfgroup.org/solutions/hdf5/

LiDARの録画データは基本的に点群の表データxフレーム数であるため、1ファイルで表現できるかつバイナリなのでIOが高速なHDF5を採用した。

HDF5出力では入力ファイルがそのまま出力ファイル名に使われる。

```
[filename].pcap -> [filename].h5
```

ファイル内の構造は以下のようになる。

```
output.h5
  - ATTRIBUTE
  - DATASETS
    - frame00000
    - frame00000
    ...
    - frame00099
```

ファイルrootのattributeとして以下の属性が付加される。

- laser number (uint32): 搭載レーザ数
- manufacturer (String): メーカー名
- model (String): センサ型番
- frequency (float32): 回転速度(Hz)
- return mode (uint32): リターンモード
  - Strongest: 0
  - Last: 1
  - Dual: 2

フレームごとの表データはファイルrootのデータセットとしてframeXXXXXという名前で格納される。
