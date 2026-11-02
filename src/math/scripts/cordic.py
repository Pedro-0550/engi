# script to generate CORDIC tables & gain
precision = 86

import mpmath as mp

mp.mp.dps = precision

print(f"const ATAN: [Real; {precision}] = [")
for i in range(precision):
    atan = mp.atan(mp.exp2(-i))
    print(
        f"    r!({atan}),",
    )
print("];\n")


print(f"const ATANH: [Real; {precision}] = [")
for i in range(precision):
    if i == 0:
        print("    r!(0),")
        continue
    atanh = mp.atanh(mp.exp2(-i))
    print(f"    r!({atanh}),")
print("];")

k_c = mp.mpf(1.0)

for i in range(precision):
    k_c /= mp.sqrt(1 + mp.mpf(2.0) ** (-2 * i))

print(f"const K_C: Real = r!({k_c});")

k_h = mp.mpf(1)
repeated = 2
iters = []

for i in range(1, precision):
    factor = 1 / mp.sqrt(1 - mp.mpf(2) ** (-2 * i))

    k_h *= factor
    iters.append(i)

    if i == (3**repeated - 1) // 2:
        k_h *= factor
        iters.append(i)
        repeated += 1

print(f"const K_H: Real = r!({k_h});")

print(f"const HYPERBOLIC_ITERS: [u8; {len(iters)}] = {iters};")
