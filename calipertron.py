import numpy as np
import matplotlib.pyplot as plt

PI = np.pi

Shift = 85 * (PI/180)
#Shift = 2 * PI / 3

# Number of positive signals on the scale.
N = 3

def signal(t, idx):
    sign = 1 if idx % (2*N) < N else -1
    shift = (idx%N) * Shift
    return sign*np.sin(t + shift)


# Combined signal when the adder is at a given position
def adder(t, position):
    position = position % (2*N)
    a = position
    b = position + N
    sum = 0;
    # check 3 sets of tracks so we don't need to consider the right side of the adder wrapping around.
    for idx in range(3*N):
        overlap = 0
        # track is entirely within adder
        if a <= idx and b >= idx + 1:
            overlap = 1

        # left edge of adder only partially overlaps this track
        if idx < a < idx + 1:
            overlap = idx + 1 - a

        # right edge of adder only partially overlaps this track
        if idx < b < idx + 1:
            overlap = b - idx

        #print((idx, overlap))
        sum += overlap * signal(t, idx)
    return sum

def main():
    t = np.linspace(0, 8*PI, 1000)

    # for i in np.linspace(0, 5, num = 6):
    #     plt.plot(t, adder(t, i), label="y(t)", linestyle="--")

    # plt.plot(t, signal(t, 0), label="f0(t)")
    # plt.plot(t, signal(t, 1), label="f1(t)")
    # plt.plot(t, signal(t, 2), label="f2(t)")

    plt.legend()
    plt.show()

if __name__ == "__main__":
    main()
