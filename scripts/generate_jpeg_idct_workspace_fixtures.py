#!/usr/bin/env python3
"""Generate full JPEG parity inputs for SIMD first-pass workspace boundaries."""
from __future__ import annotations

import argparse
import hashlib
from io import BytesIO
from pathlib import Path

from PIL import Image, __version__ as pillow_version
from generate_jpeg_jcoef_boundary_fixtures import fixture

ROOT = Path(__file__).resolve().parents[1]
# Width, height, category, Al, negative, refinements, input and Pillow pixel SHA.
EXPECTED = {
    "progressive_idct_8x8_cat1_al0_positive_first": (
        8, 8, 1, 0, False, False,
        "73ff44ded7cd0536bd3acc076bae607edb17af885e24d6ae8b6cf5b210b1b17c",
        "1df1b7ce1fd8fcbe20cde61646875e54fe38d8945ea7911afd59e025cc520a68",
    ),
    "progressive_idct_8x8_cat1_al0_negative_first": (
        8, 8, 1, 0, True, False,
        "ceff9e8587d463ea2014b6be5cceb2dbd94f94f9c5b8049db329a538ceb2de32",
        "1df1b7ce1fd8fcbe20cde61646875e54fe38d8945ea7911afd59e025cc520a68",
    ),
    "progressive_idct_8x8_cat1_al12_positive_first": (
        8, 8, 1, 12, False, False,
        "305864926e48dfe573b872c8aeeef31bf4f1c16e078bc3d907beb5b4b76670ef",
        "f7d5c667e7c2e0b44c4c5f00b712048e052f4386448b90d36da393584079b847",
    ),
    "progressive_idct_8x8_cat1_al12_negative_first": (
        8, 8, 1, 12, True, False,
        "d009f589757d5aab000c96a59dc3555c9f2115bdd1ed54ee747d417673a267cf",
        "733ab3e3e2e3cd83729cc89eca97a2362c127227496a9d57d39160ee6de1d6e3",
    ),
    "progressive_idct_8x8_cat1_al13_positive_first": (
        8, 8, 1, 13, False, False,
        "9fb24f78d0cc57e9e4a2833107c9d49b3ae202fed598043d531e825c0dc2df85",
        "5b1369a8e22a8d0d231c3439cfb22c82241121a8dfaed1a1b37e23d1d2d88491",
    ),
    "progressive_idct_8x8_cat1_al13_positive_refined": (
        8, 8, 1, 13, False, True,
        "01a1def580628345c71f7c2eb128f935aad7515df11ca2e933eac082733837d9",
        "1df1b7ce1fd8fcbe20cde61646875e54fe38d8945ea7911afd59e025cc520a68",
    ),
    "progressive_idct_8x8_cat1_al13_negative_first": (
        8, 8, 1, 13, True, False,
        "3dde5e8554191a5621cecc735a48ff5c576b84aa43ab9b43074b322b32e95b35",
        "5b1369a8e22a8d0d231c3439cfb22c82241121a8dfaed1a1b37e23d1d2d88491",
    ),
    "progressive_idct_8x8_cat1_al13_negative_refined": (
        8, 8, 1, 13, True, True,
        "cfa7a152b1acca41fefdd57279237371450dd1e9983b745575de8fa29d6ddce4",
        "1df1b7ce1fd8fcbe20cde61646875e54fe38d8945ea7911afd59e025cc520a68",
    ),
    "progressive_idct_8x8_cat3_al0_positive_first": (
        8, 8, 3, 0, False, False,
        "ea3da7d357daf622568abf392ec4302a565c1b005f79cf2bed6ca92eff8cb494",
        "b5fc2c57ee7922296caf1a1e4559c1461047e465a67642dc7864907f99c366f1",
    ),
    "progressive_idct_8x8_cat3_al0_negative_first": (
        8, 8, 3, 0, True, False,
        "23d18a8e07a6597db1880e0da4ccd4ecc2596288de6b9f4471710877a833dbd6",
        "7abadc7de33d8a7eb55009535a2973d9bde2e134fd8d8489efffc5a5b721942b",
    ),
    "progressive_idct_8x8_cat3_al12_positive_first": (
        8, 8, 3, 12, False, False,
        "bdd7f48d9c47f4d591147a7d361d1a5b58c8a717d6ee42fa1f2362d027c6434d",
        "733ab3e3e2e3cd83729cc89eca97a2362c127227496a9d57d39160ee6de1d6e3",
    ),
    "progressive_idct_8x8_cat3_al12_positive_refined": (
        8, 8, 3, 12, False, True,
        "98a4099ae80998005a66ddc3ac6af43920836446f8f46741ba65ecc7779df4f7",
        "1df1b7ce1fd8fcbe20cde61646875e54fe38d8945ea7911afd59e025cc520a68",
    ),
    "progressive_idct_8x8_cat3_al12_negative_first": (
        8, 8, 3, 12, True, False,
        "a247d2920cd93b891d9ccff6852e881b0e7c51b990fd05223b477e57af3f5ff7",
        "f7d5c667e7c2e0b44c4c5f00b712048e052f4386448b90d36da393584079b847",
    ),
    "progressive_idct_8x8_cat3_al12_negative_refined": (
        8, 8, 3, 12, True, True,
        "dd962a4bd7294e5e838d456f172b6eb17e73468e523cd1c822ab6ffd4d143bcd",
        "8667e718294e9e0df1d30600ba3eeb201f764aad2dad72748643e4a285e1d1f7",
    ),
    "progressive_idct_8x8_cat3_al13_positive_first": (
        8, 8, 3, 13, False, False,
        "b793d2c64b5a511856713035d6e408a17e0c46f183451bfc9b54473550e13fa3",
        "5b1369a8e22a8d0d231c3439cfb22c82241121a8dfaed1a1b37e23d1d2d88491",
    ),
    "progressive_idct_8x8_cat3_al13_positive_refined": (
        8, 8, 3, 13, False, True,
        "9fd11d31c7f013c3dbe4b24ab3876e7a8dd3e2f3ad8c29ee3099ac8cb25cea79",
        "1df1b7ce1fd8fcbe20cde61646875e54fe38d8945ea7911afd59e025cc520a68",
    ),
    "progressive_idct_8x8_cat3_al13_negative_first": (
        8, 8, 3, 13, True, False,
        "bf85133739ff738afd375d3da6ea25523a22a177db14034aed7d397cf138a50b",
        "5b1369a8e22a8d0d231c3439cfb22c82241121a8dfaed1a1b37e23d1d2d88491",
    ),
    "progressive_idct_8x8_cat3_al13_negative_refined": (
        8, 8, 3, 13, True, True,
        "58493cda07f1514ed9081fd4c1320d6909bb786787f04fc80c8b814ba28117d9",
        "1df1b7ce1fd8fcbe20cde61646875e54fe38d8945ea7911afd59e025cc520a68",
    ),
    "progressive_idct_8x8_cat15_al0_positive_first": (
        8, 8, 15, 0, False, False,
        "7afe5b192c8105180e0dc79271e0c0c0c216e184c273fb135550a62096527f43",
        "1df1b7ce1fd8fcbe20cde61646875e54fe38d8945ea7911afd59e025cc520a68",
    ),
    "progressive_idct_8x8_cat15_al0_negative_first": (
        8, 8, 15, 0, True, False,
        "d4a82c5734fd6f0f411e3ff5bfe83de526f20249fd616c5bb96e703d0d87441f",
        "1df1b7ce1fd8fcbe20cde61646875e54fe38d8945ea7911afd59e025cc520a68",
    ),
    "progressive_idct_8x8_cat15_al13_positive_first": (
        8, 8, 15, 13, False, False,
        "f026be2537a7d13c0c86a3a3dc8e159d6402a2d3805a115973361fd410721843",
        "5b1369a8e22a8d0d231c3439cfb22c82241121a8dfaed1a1b37e23d1d2d88491",
    ),
    "progressive_idct_8x8_cat15_al13_positive_refined": (
        8, 8, 15, 13, False, True,
        "bd4e601e02a2469a1e8005e4b13a416b1ae3eb58c0f1506906590e0daa0ede81",
        "1df1b7ce1fd8fcbe20cde61646875e54fe38d8945ea7911afd59e025cc520a68",
    ),
    "progressive_idct_8x8_cat15_al13_negative_first": (
        8, 8, 15, 13, True, False,
        "e533622cdcf0afad0429a840181d6ccc26c1f9f35c56ce1e2275734c182a63a1",
        "5b1369a8e22a8d0d231c3439cfb22c82241121a8dfaed1a1b37e23d1d2d88491",
    ),
    "progressive_idct_8x8_cat15_al13_negative_refined": (
        8, 8, 15, 13, True, True,
        "a2100ed3ebdf53ed98561b413fd0d9522f7c32f70ec3c8577c9049284c92fd27",
        "1df1b7ce1fd8fcbe20cde61646875e54fe38d8945ea7911afd59e025cc520a68",
    ),
    "progressive_idct_17x9_cat1_al0_positive_first": (
        17, 9, 1, 0, False, False,
        "c13a38bf7c0f913a66a2c043460e6739c7530e3b73607c1136f691fd6148fc0f",
        "58c65f92ad73f9fe4e2f0d588c0ab9fe33f1a3ed4d971c80f5c22303f1f7dc1f",
    ),
    "progressive_idct_17x9_cat1_al0_negative_first": (
        17, 9, 1, 0, True, False,
        "17a1cba59d9c0bb94cae26c95f46690a98cd2d7e7b4a2b97cf742deec1467c49",
        "56de78f344a294a6f6e38ca132e29b9924a077ff0930de2c1c9da964136ced27",
    ),
    "progressive_idct_17x9_cat1_al12_positive_refined": (
        17, 9, 1, 12, False, True,
        "526d313403c9b94881eba4805d10c075bf57558c2d1f133de2b67b96845efe1d",
        "fd5eef88cd4b62136901faba5071ba7057253ebfe382cf47f6667d38bbaf14bd",
    ),
    "progressive_idct_17x9_cat1_al12_negative_refined": (
        17, 9, 1, 12, True, True,
        "47e7f45c5236687486be4ba91290dae7cba6a9a98f0ab9f951accd060772a698",
        "97c5a7acfa17ec19e61779bf4eca863c8cab5a5b480a23b93884e03aa07a2612",
    ),
    "progressive_idct_17x9_cat1_al13_positive_refined": (
        17, 9, 1, 13, False, True,
        "3fedf0a4d6b2c36df12740d19ad6c87d1b5aa26636271de4bdc50b715ba268da",
        "b2ae50ea63d2e41098f9f40423a9090504bbc240ab834c725298ed45aefe3d87",
    ),
    "progressive_idct_17x9_cat1_al13_negative_refined": (
        17, 9, 1, 13, True, True,
        "08e836ac655d1904fa0cf2bd062c7d4f514baf9b3fe3c921889cb7fe3ed71ee8",
        "b2ae50ea63d2e41098f9f40423a9090504bbc240ab834c725298ed45aefe3d87",
    ),
    "progressive_idct_17x9_cat3_al0_positive_first": (
        17, 9, 3, 0, False, False,
        "0677c41e25b14a6dc55984f7da0b5633253f946481965d8253e6d1f94a4a387c",
        "b4702012f02d3da264953c19dcb01d628c986af67c0dea3a464897ccc804283f",
    ),
    "progressive_idct_17x9_cat3_al0_negative_first": (
        17, 9, 3, 0, True, False,
        "879fae335de0fb96051805c41c67b792c82d655c6dcaea2fd40d94611c20882b",
        "66f2321f1621e0ce2125d787a5c21ddc5bb966622d70ae4aa20a5ac781340971",
    ),
    "progressive_idct_17x9_cat3_al12_positive_refined": (
        17, 9, 3, 12, False, True,
        "b91af45825c4ff3ef64dcd808fd9a89d07924e60ab1186547d76745335cfa21a",
        "ce815f170ac2530ee46c220fdd640dd7435e8898213538b66d6be10f8fd45f25",
    ),
    "progressive_idct_17x9_cat3_al12_negative_refined": (
        17, 9, 3, 12, True, True,
        "4e3158512219e952c604dfae6f894b5c26a6782816c3a18b05865a6ce21070fe",
        "29246e5e0662eab0e84de8e8593504b75ffa647f99a20a307423fa07f328f779",
    ),
    "progressive_idct_17x9_cat3_al13_positive_refined": (
        17, 9, 3, 13, False, True,
        "7228d1f340889633947991295544fad0be806b2fc2d51f2b37c77d65736b76cf",
        "b2ae50ea63d2e41098f9f40423a9090504bbc240ab834c725298ed45aefe3d87",
    ),
    "progressive_idct_17x9_cat3_al13_negative_refined": (
        17, 9, 3, 13, True, True,
        "0b71cec35f111a3e1c3a2608677933622a503fca10a4c9ae7871f1d94b8dafab",
        "b2ae50ea63d2e41098f9f40423a9090504bbc240ab834c725298ed45aefe3d87",
    ),
    "progressive_idct_17x9_cat15_al0_positive_first": (
        17, 9, 15, 0, False, False,
        "c3bd2f0b0d93e67a2dba17ac33f848c20a95579d3f8110310af67fc9cd032fe2",
        "56de78f344a294a6f6e38ca132e29b9924a077ff0930de2c1c9da964136ced27",
    ),
    "progressive_idct_17x9_cat15_al0_negative_first": (
        17, 9, 15, 0, True, False,
        "cfe6a5ed011429e5afc26035f66206fe3162e70bb25771e4529685c4159e30bb",
        "58c65f92ad73f9fe4e2f0d588c0ab9fe33f1a3ed4d971c80f5c22303f1f7dc1f",
    ),
    "progressive_idct_17x9_cat15_al12_positive_refined": (
        17, 9, 15, 12, False, True,
        "a9769320739688929bd7245c120817345d7cacc41d9abf02c8eacca40c4374a9",
        "97c5a7acfa17ec19e61779bf4eca863c8cab5a5b480a23b93884e03aa07a2612",
    ),
    "progressive_idct_17x9_cat15_al12_negative_refined": (
        17, 9, 15, 12, True, True,
        "67c5985b9a32c9068b48d8c86e14854705e6aba4814c90e74003d1c855ecf1a4",
        "fd5eef88cd4b62136901faba5071ba7057253ebfe382cf47f6667d38bbaf14bd",
    ),
    "progressive_idct_17x9_cat15_al13_positive_refined": (
        17, 9, 15, 13, False, True,
        "dadec4c76988d37f963b08c026287b70c4f7a088ef917f6d25a652ddf984e111",
        "b2ae50ea63d2e41098f9f40423a9090504bbc240ab834c725298ed45aefe3d87",
    ),
    "progressive_idct_17x9_cat15_al13_negative_refined": (
        17, 9, 15, 13, True, True,
        "2d0fb3da874ec0afe1ec1332c90596695e296289260eb6b219bb2fc81085970c",
        "b2ae50ea63d2e41098f9f40423a9090504bbc240ab834c725298ed45aefe3d87",
    ),
}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="verify without writing")
    args = parser.parse_args()
    if pillow_version != "12.2.0":
        raise RuntimeError(f"expected Pillow 12.2.0, found {pillow_version}")
    for name, parameters in EXPECTED.items():
        width, height, category, al, negative, refined, input_sha, pixel_sha = parameters
        encoded = fixture(width, height, category, negative, al, refined)
        if hashlib.sha256(encoded).hexdigest() != input_sha:
            raise RuntimeError(f"JPEG input bytes changed: {name}")
        with Image.open(BytesIO(encoded)) as image:
            image.verify()
        with Image.open(BytesIO(encoded)) as image:
            image.load()
            if image.size != (width, height) or image.mode != "L":
                raise RuntimeError(f"Pillow metadata changed: {name}")
            if hashlib.sha256(image.tobytes()).hexdigest() != pixel_sha:
                raise RuntimeError(f"Pillow pixels changed: {name}")
        path = ROOT / "tests/fixtures/input/images/jpeg" / (name + ".jpg")
        if path.exists() and path.read_bytes() != encoded:
            raise RuntimeError(f"refusing to replace different fixture bytes: {name}")
        if args.check:
            if not path.is_file():
                raise RuntimeError(f"fixture is missing: {name}")
        else:
            path.write_bytes(encoded)
        print(f"{'Checked' if args.check else 'Generated'} {name}")


if __name__ == "__main__":
    main()
