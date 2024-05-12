import numpy as np
import matplotlib.pyplot as plt

Pi = np.pi
Shift = 85 * (Pi/180)
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


def first_zero_crossing(t, y):
    sign_changes = np.where(np.diff(y > 0))[0]

    # filter for positive to negative transitions
    zero_crossings = t[sign_changes][y[sign_changes] > 0]
    return zero_crossings[0]


def main():

    t = np.linspace(0, 8*Pi, 5000)
    res = [];
    for x in np.linspace(0, 10, 100):
      y = adder(t, x)
      first_zero = first_zero_crossing(t, y)
      res.append([x, first_zero])
    res = np.array(res)
    plt.plot(res[:,0], res[:,1])
    plt.show()


if __name__ == "__main__":
    main()
