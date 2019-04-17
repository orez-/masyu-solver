import collections
import contextlib
import dataclasses
import enum
import textwrap


WHITE = object()
BLACK = object()
SYMBOL_LOOKUP = {'o': WHITE, '●': BLACK}


class Direction(enum.Enum):
    Up = (0, -1)
    Right = (1, 0)
    Down = (0, 1)
    Left = (-1, 0)

    def opposite(self):
        x, y = self.value
        return Direction((-x, -y))

    def move(self, x, y):
        dx, dy = self.value
        return (x + dx, y + dy)


@contextlib.contextmanager
def does_raise(*excs):
    """
    Context which will suppress the given exception types, and return caught exceptions in a list.

    This can be used to check if some expected exception was raised or not, without having to
    handle it immediately in an `except` block.

    Note that before the context has exited the value of the returned object is undefined.
    """
    caught_excs = []
    try:
        yield caught_excs
    except tuple(excs) as exc:
        caught_excs.append(exc)


class ContradictionException(Exception):
    """The attempted operation would result in a contradiction in board state!"""


@dataclasses.dataclass
class CellLine:
    is_set: {Direction}
    cannot_set: {Direction}

    def set_direction(self, direction):
        """
        Set a line in the direction given and return the resultant CellLine.

        Raises `ContradictionException` if this is an invalid action.
        """
        if direction in self.is_set:
            return self
        if direction in self.cannot_set:
            raise ContradictionException(f"Can't set {direction} on cell {self}")

        is_set = self.is_set | {direction}
        cannot_set = self.cannot_set

        if len(is_set) == 2:
            cannot_set = frozenset(Direction) - is_set
        elif len(cannot_set) == 2:
            is_set = frozenset(Direction) - cannot_set
        return CellLine(
            is_set=frozenset(is_set),
            cannot_set=frozenset(cannot_set),
        )

    def disallow_direction(self, direction):
        """
        Reject the possibility of a line in the direction given, and return the resultant CellLine.

        Raises `ContradictionException` if this is an invalid action.
        """
        if direction in self.cannot_set:
            return self
        if direction in self.is_set:
            raise ContradictionException(f"Can't disallow {direction} on cell {self}")

        cannot_set = self.cannot_set | {direction}
        is_set = self.is_set

        # Need to make sure there _should_ be a line here.
        # Each cell does not necessarily contain a line!
        if len(cannot_set) == 2 and len(is_set) == 1:
            is_set = set(Direction) - cannot_set
        elif len(cannot_set) == 3:
            cannot_set = Direction
        return CellLine(
            is_set=frozenset(is_set),
            cannot_set=frozenset(cannot_set),
        )

    def get_through(self):
        """
        Return the transformation of this CellLine assuming the line will pass straight through.

        If there is not enough information to apply this transformation the original CellLine
        is returned instead.

        If this transformation is invalid `ContradictionException` is raised.
        """
        num_set = len(self.is_set)
        if num_set == 2:
            one, other = self.is_set
            if one.opposite() != other:
                raise ContradictionException(f"{self} is already bent!")
            return self
        if num_set == 1:
            one, = self.is_set
            return self.set_direction(one.opposite())
        assert num_set == 0, f"expected none set, found {num_set} ({self.is_set})"

        num_cannot_set = len(self.cannot_set)
        if num_cannot_set == 1:
            one, = self.cannot_set
            return self.disallow_direction(one.opposite())
        if num_cannot_set == 2:
            is_set = one, other = frozenset(Direction) - self.cannot_set
            if one.opposite() != other:
                raise ContradictionException(f"No straight path exists through {self}")
            return CellLine(
                is_set=is_set,
                cannot_set=self.cannot_set,
            )
        if num_cannot_set == 4:
            raise ContradictionException(f"{self} must be blank")

        assert num_cannot_set == 0, (
            f"expected no `cannot_set`, found {num_cannot_set} ({self.cannot_set})")

        # We know nothing about this cell.
        return self

    def get_bent(self):  # 💁‍♀
        """
        Return the transformation of this CellLine assuming the line will pass straight through.

        If there is not enough information to apply this transformation any possible constraints
        will be applied instead. If no constraints can be applied the original CellLine is returned
        instead.

        If this transformation is invalid `ContradictionException` is raised.
        """
        num_set = len(self.is_set)
        if num_set == 2:
            one, other = self.is_set
            if one.opposite() == other:
                raise ContradictionException(f"{self} is already straight-through!")
            return self
        if num_set == 1:
            one, = self.is_set
            return self.disallow_direction(one.opposite())

        num_cannot_set = len(self.cannot_set)
        if num_cannot_set == 1:
            one, = self.cannot_set
            return self.set_direction(one.opposite())
        if num_cannot_set == 2:
            is_set = one, other = frozenset(Direction) - self.cannot_set
            if one.opposite() == other:
                raise ContradictionException(f"No bent path exists through {self}")
            return CellLine(
                is_set=is_set,
                cannot_set=self.cannot_set,
            )
        if num_cannot_set == 4:
            raise ContradictionException(f"{self} must be blank")

        # 2-case should be handled by `num_set == 2` above.
        assert num_cannot_set == 0, (
            f"expected no `cannot_set`, found {num_cannot_set} ({self.cannot_set})")

        # We know nothing about this cell.
        return self

    def is_done(self):
        return len(self.is_set) + len(self.cannot_set) == 4


@dataclasses.dataclass
class LineSegment:
    ...


@dataclasses.dataclass
class Board:
    width: int
    height: int
    circles: {(int, int): bool}
    cell_lines: {(int, int): CellLine} = None
    line_segments: [LineSegment] = dataclasses.field(init=False)

    def __post_init__(self):
        if self.cell_lines is None:
            self.cell_lines = {
                (x, y): CellLine(
                    is_set=frozenset(),
                    cannot_set=self._edges_at(x, y),
                )
                for y in range(self.height)
                for x in range(self.width)
            }
        self.line_segments = []

    def _edges_at(self, x, y):
        edges = set()
        if x == 0:
            edges.add(Direction.Left)
        if y == 0:
            edges.add(Direction.Up)
        if y == self.height - 1:
            edges.add(Direction.Down)
        if x == self.width - 1:
            edges.add(Direction.Right)
        return frozenset(edges)

    def set_direction(self, x, y, direction):
        old_cell = self.cell_lines[x, y]
        new_cell = old_cell.set_direction(direction)
        if new_cell == old_cell:
            return self
        return self._propagate_change({(x, y): new_cell})

    def set_through(self, x, y):
        old_cell = self.cell_lines[x, y]
        new_cell = old_cell.get_through()
        if new_cell == old_cell:
            return self
        return self._propagate_change({(x, y): new_cell})

    def set_bent(self, x, y):
        old_cell = self.cell_lines[x, y]
        new_cell = old_cell.get_bent()
        if new_cell == old_cell:
            return self
        return self._propagate_change({(x, y): new_cell})

    def _propagate_change(self, changes):
        # May raise ContradictionException
        cell_lookup = collections.ChainMap(changes, self.cell_lines)

        positions = collections.deque(changes)
        while positions:
            x, y = positions.popleft()
            for direction in changes[x, y].is_set:
                mx, my = direction.move(x, y)
                # This lookup should always succeed actually:
                # we should never run the line off the board.
                old_cell = cell_lookup[mx, my]
                new_cell = old_cell.set_direction(direction.opposite())
                if new_cell == old_cell:
                    continue
                positions.append((mx, my))
                changes[mx, my] = new_cell

            for direction in changes[x, y].cannot_set:
                mx, my = direction.move(x, y)
                old_cell = cell_lookup.get((mx, my))
                if not old_cell:
                    continue
                new_cell = old_cell.disallow_direction(direction.opposite())
                if new_cell == old_cell:
                    continue
                positions.append((mx, my))
                changes[mx, my] = new_cell
        return Board(
            width=self.width,
            height=self.height,
            circles=self.circles,
            cell_lines=dict(cell_lookup),
        )


# solver shit


def apply_white(board, x, y):
    board = board.set_through(x, y)

    cell_set = board.cell_lines[x, y].is_set
    if len(cell_set) != 2:
        # TODO: Could try bending in both directions: if one configuration
        # can't bend in either then it's the other way.
        return board

    left, right = cell_set
    lx, ly = left.move(x, y)
    with does_raise(ContradictionException) as left_must_straight:
        bend_left = board.set_bent(lx, ly)

    rx, ry = right.move(x, y)
    with does_raise(ContradictionException) as right_must_straight:
        bend_right = board.set_bent(rx, ry)

    if left_must_straight and right_must_straight:
        raise ContradictionException(f"Cannot bend either end of the white circle at {x}, {y}")

    if not left_must_straight and not right_must_straight:
        # We don't really know anything: either could bend.
        return board

    # We know something at this point though: only one may bend!
    # Bend that one!!
    assert bool(left_must_straight) != bool(right_must_straight), "Expected exactly one may bend"
    return bend_right if left_must_straight else bend_left


def apply_black(board, x, y):
    board = board.set_bent(x, y)
    cell = board.cell_lines[x, y]

    # extend existing lines
    for direction in cell.is_set:
        mx, my = direction.move(x, y)
        board = board.set_through(mx, my)

    if cell.is_done():
        return board

    could_dirs = set()
    for direction in frozenset(Direction) - cell.is_set - cell.cannot_set:
        mx, my = direction.move(x, y)
        with contextlib.suppress(ContradictionException):
            new_board = board.set_direction(x, y, direction)
            new_board.set_through(mx, my)
            could_dirs.add(direction)

    for direction in could_dirs:
        if direction.opposite() not in could_dirs:
            mx, my = direction.move(x, y)
            board = board.set_direction(x, y, direction)
            board = board.set_through(mx, my)

    return board


def solve(board):
    old_board = None
    while old_board != board:
        old_board = board
        for (x, y), circle in board.circles.items():
            if circle is WHITE:
                board = apply_white(board, x, y)
            else:
                board = apply_black(board, x, y)
    return board


# display shit

def board_from_string(board_str):
    board_lines = textwrap.dedent(board_str.rstrip().strip('\n')).split('\n')

    return Board(
        width=len(board_lines[0]),
        height=len(board_lines),
        circles={
            (x, y): SYMBOL_LOOKUP[elem]
            for y, row in enumerate(board_lines)
            for x, elem in enumerate(row)
            if elem != '.'
        },
    )


def print_board(board):
    board_str = []
    for y in range(board.height):
        for x in range(board.width):
            if (x, y) in board.circles:
                board_str.append('●' if board.circles[x, y] == BLACK else 'o')
            else:
                board_str.append('.')
        board_str.append('\n')
    return ''.join(board_str)


af = '\x1b[38;5;{}m'.format
CLEAR = '\x1b[0m'

INNER_CELL_LINE = {
    frozenset({Direction.Down, Direction.Up}): '│',
    frozenset({Direction.Left, Direction.Right}): '─',
    frozenset({Direction.Left, Direction.Down}): '┐',
    frozenset({Direction.Left, Direction.Up}): '┘',
    frozenset({Direction.Right, Direction.Up}): '└',
    frozenset({Direction.Right, Direction.Down}): '┌',
}

def print_big_board(board):
    # ┌┬┐
    # ├┼┤
    # └┴┘
    # ─│

    GRAY = af(8)
    board_str = [GRAY, '┌', '┬'.join('─' * board.width), '┐\n']

    for row in range(board.height):
        board_str.extend(['│', CLEAR])
        for col in range(board.width):
            cell = board.cell_lines[col, row]
            if (col, row) in board.circles:
                board_str.append('●' if board.circles[col, row] == BLACK else 'o')
            else:
                board_str.append(INNER_CELL_LINE.get(cell.is_set, ' '))
            if Direction.Right in cell.is_set:
                board_str.append('─')
            else:
                board_str.extend([GRAY, '│', CLEAR])

        if row == board.height - 1:
            board_str.extend([GRAY, '\n└', '┴'.join('─' * board.width), '┘', CLEAR])
        else:
            board_str.extend([GRAY, '\n├'])
            for col in range(board.width):
                cell = board.cell_lines[col, row]
                if Direction.Down in cell.is_set:
                    board_str.extend([CLEAR, '│', GRAY])
                else:
                    board_str.append('─')

                board_str.append('┤' if col == board.width - 1 else '┼')

        board_str.append('\n')
    return ''.join(board_str)


def main():
    # < redacted >
    board_str = ...
    board = board_from_string(board_str)
    print(print_board(board))

    board = solve(board)
    print(print_big_board(board))


if __name__ == '__main__':
    main()