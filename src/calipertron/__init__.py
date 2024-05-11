import numpy as np
import matplotlib.pyplot as plt

PI = np.pi
#Shift = 85 * (PI/180)
Shift = 2 * PI / 3
N = 3

def signal(t, idx):
    sign = 1 if idx < N else -1
    return sign*np.sin(t + idx*Shift)


def adder(position, t):
    a = position % (2*N)
    b = (position + N) % (2*N)
    sum = 0;
    for idx in range(2*N):
        overlap = 0
        # track is entirely within adder
        if a < idx and ((idx - a) < N):
            overlap = 1

        # left edge of adder only partially overlaps this track
        if idx < a < idx + 1:
            overlap = a - idx

        # right edge of adder only partially overlaps this track
        if idx < b < idx + 1:
            overlap = b - idx
        print(overlap)
        sum += overlap * signal(t, idx)
    return sum

def main():
    t = np.linspace(0, 8*PI, 1000)

    # for i in np.linspace(0, 5, num = 6):
    #     plt.plot(t, adder(i, t), label="y(t)", linestyle="--")

    # plt.plot(t, signal(t, 0), label="f0(t)")
    # plt.plot(t, signal(t, 1), label="f1(t)")
    # plt.plot(t, signal(t, 2), label="f2(t)")

    plt.legend()
    plt.show()

if __name__ == "__main__":
    main()
