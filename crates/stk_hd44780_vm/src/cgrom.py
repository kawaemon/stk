data = """
________________
________________
_!"#$%&'()*+,-./
0123456789:;<=>?
@ABCDEFGHIJKLMNO
PQRSTUVWXYZ[¥]^_
`abcdefghijklmno
pqrstuvwxyz(|)→←
________________
________________
_｡｢｣`･ｦｧｨｩｪｫｬｭｮｯ
ｰｱｲｳｴｵｶｷｸｹｺｻｼｽｾｿ
ﾀﾁﾂﾃﾄﾅﾆﾇﾈﾉﾊﾋﾌﾍﾎﾏ
ﾐﾑﾒﾓﾔﾕﾖﾗﾘﾙﾚﾛﾜﾝﾞﾟ
αäβεμερg√┘jx€Lüö
pqθ∞ΩüΣπx̄y千万円÷█
"""
# FIXME: 下の 2 列に正しくない文字がいくつかあり、何を入れていいのか分からない
# ┘ ← -1 乗っぽい

for l in data.splitlines():
    for c in l:
        if c == '_':
            print("' '", end="")
        else:
            print(f"'{c}'", end="")
        print(", ", end="")
    print()

