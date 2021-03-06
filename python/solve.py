import collections
import contextlib
import dataclasses
import enum
import sys
import textwrap
import weakref
from unittest.mock import sentinel

import attr
import frozendict


WHITE = sentinel.WHITE
BLACK = sentinel.BLACK
SYMBOL_LOOKUP = {'o': WHITE, '●': BLACK}


class Direction(enum.Enum):
    Up = (0, -1)
    Right = (1, 0)
    Down = (0, 1)
    Left = (-1, 0)

    def opposite(self):
        x, y = self.value
        return Direction((-x, -y))

    def turn_right(self):
        x, y = self.value
        return Direction((-y, x))

    def turn_left(self):
        x, y = self.value
        return Direction((y, -x))

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


class LoopException(Exception):
    """We've created a loop! Either we've solved the board, or we've created a contradiction."""

    def __init__(self, loop_coords):
        super().__init__(loop_coords)
        self._loop_coords = loop_coords

    def validate_solved(self, circles):
        """Ensure that the loop touches all circles."""
        if not all(circle_coord in self._loop_coords for circle_coord in circles):
            raise ContradictionException("Closed loop does not contain all circles") from self


class SolvedException(Exception):
    """The board is solved!"""

    def __init__(self, board):
        super().__init__()
        self.board = board


@dataclasses.dataclass(frozen=True)
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
        return CellLine(is_set=frozenset(is_set), cannot_set=frozenset(cannot_set))

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
        return CellLine(is_set=frozenset(is_set), cannot_set=frozenset(cannot_set))

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
            cannot_set = frozenset({one, one.opposite()})
            return CellLine(is_set=frozenset(Direction) - cannot_set, cannot_set=cannot_set)
        if num_cannot_set == 2:
            is_set = one, other = frozenset(Direction) - self.cannot_set
            if one.opposite() != other:
                raise ContradictionException(f"No straight path exists through {self}")
            return CellLine(is_set=is_set, cannot_set=self.cannot_set)
        if num_cannot_set == 4:
            raise ContradictionException(f"{self} must be blank")

        assert (
            num_cannot_set == 0
        ), f"expected no `cannot_set`, found {num_cannot_set} ({self.cannot_set})"

        # We know nothing about this cell.
        return self

    def get_bent(self):  # 💁‍♀
        """
        Return the transformation of this CellLine assuming the line will bend.

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
            return CellLine(is_set=is_set, cannot_set=self.cannot_set)
        if num_cannot_set == 4:
            raise ContradictionException(f"{self} must be blank")

        # 2-case should be handled by `num_set == 2` above.
        assert (
            num_cannot_set == 0
        ), f"expected no `cannot_set`, found {num_cannot_set} ({self.cannot_set})"

        # We know nothing about this cell.
        return self

    def is_done(self):
        return len(self.is_set) + len(self.cannot_set) == 4

    def could_set(self):
        return frozenset(Direction) - self.is_set - self.cannot_set

    def other_out(self, direction):
        other = set(self.is_set - {direction})
        return other.pop() if other else None


@attr.s(frozen=True)
class LineSegment:
    start: (int, int) = attr.ib()
    start_direction: Direction = attr.ib()
    end: (int, int) = attr.ib()
    end_direction: Direction = attr.ib()
    contains: frozenset = attr.ib()

    def other_end(self, coord):
        if coord == self.start:
            return self.end, self.end_direction
        return self.start, self.start_direction


def _loop_path(coord, direction, cell_lines):
    x, y = coord
    # while you've got somewhere to go, go there, poop out where you are,
    # and figure out where to go next.
    while direction:
        x, y = direction.move(x, y)
        direction = direction.opposite()
        yield (x, y), direction
        direction = cell_lines[x, y].other_out(direction)


def _extend_line_segments(line_segments, cell_lines):
    """Return an updated list of all known line segments with any additions they have accrued."""
    coord_lookup = {coord: seg for seg in line_segments for coord in [seg.start, seg.end]}
    seen_segs = set()
    new_segs = []
    for segment in line_segments:
        if segment in seen_segs:
            continue
        seen_segs.add(segment)

        loop = set(segment.contains)
        changed_ends = {}

        segment_ends = [
            ('start', segment.start, segment.start_direction),
            ('end', segment.end, segment.end_direction),
        ]
        for side, coord, segment_direction in segment_ends:
            # If something added on to one end
            cell = cell_lines[coord]
            if len(cell.is_set) == 1:
                continue
            # Follow that end
            next_dir = cell.other_out(segment_direction)
            iterator = SwappableIterator(_loop_path(coord, next_dir, cell_lines))
            # It's *extremely* intentional that we're redefining `coord` here,
            # and I'm only *slightly* remorseful about it.
            for coord, next_dir in iterator:  # pylint: disable=redefined-outer-name
                if coord in coord_lookup:
                    merge_seg = coord_lookup[coord]
                    # Check if we looped back on ourselves.
                    if merge_seg == segment:
                        raise LoopException(loop)
                    # Otherwise, quick consume the line segment
                    seen_segs.add(merge_seg)
                    loop |= merge_seg.contains
                    coord, next_dir = merge_seg.other_end(coord)
                    move_dir = cell_lines[coord].other_out(next_dir)
                    iterator.swap(_loop_path(coord, move_dir, cell_lines))
                else:
                    loop.add(coord)
            changed_ends[side] = coord
            changed_ends[f"{side}_direction"] = next_dir

        new_segs.append(
            attr.evolve(segment, contains=frozenset(loop), **changed_ends) if changed_ends else segment
        )
    return tuple(new_segs)


def _discover_line_segments(cell_lines, seen=()):
    """
    Find new line segments based on `cell_lines`.

    Do not check cells with coordinates in `seen`.
    """
    seen = set(seen)
    line_segments = []
    for coord, cell in cell_lines.items():
        # Skip cells we've already seen, or that have no lines.
        if coord in seen or not cell.is_set:
            continue

        loop = {coord}
        # Backward, and check for closed loop.
        # If there is no backward we're already at the start
        start = coord
        if len(cell.is_set) == 1:
            [forward_dir] = [back_dir] = cell.is_set
        else:
            [forward_dir, back_dir] = cell.is_set
            for start, back_dir in _loop_path(coord, back_dir, cell_lines):
                if start in loop:
                    # We've got a closed loop! This is exceptional!
                    # We're definitely either done or wrong.
                    # Either way, stop what you're doing and say something!
                    raise LoopException(loop)
                loop.add(start)

        # Forward!
        end = coord
        for end, forward_dir in _loop_path(coord, forward_dir, cell_lines):
            loop.add(end)

        seen |= loop
        line_segments.append(
            LineSegment(
                start=start,
                start_direction=back_dir,
                end=end,
                end_direction=forward_dir,
                contains=frozenset(loop),
            )
        )

    return tuple(line_segments)


@attr.s(frozen=True, repr=False)
class Board:
    width: int = attr.ib()
    height: int = attr.ib()
    circles: {(int, int): bool} = attr.ib()
    cell_lines: {(int, int): CellLine} = attr.ib()
    # Bookkeep-y list of line segments. Constructed from `cell_lines`,
    # but nice to track for optimization purposes.
    line_segments: [LineSegment] = attr.ib(cmp=False)

    @cell_lines.default
    def _(self):
        return frozendict.frozendict({
            (x, y): CellLine(is_set=frozenset(), cannot_set=self._edges_at(x, y))
            for y in range(self.height)
            for x in range(self.width)
        })

    @line_segments.default
    def _(self):
        return _discover_line_segments(self.cell_lines)

    def evolve(self, cell_lines):
        cell_lines = frozendict.frozendict(cell_lines)
        try:
            line_segments = _extend_line_segments(self.line_segments, cell_lines)
            seen = frozenset().union(*(seg.contains for seg in line_segments))
            line_segments += _discover_line_segments(cell_lines, seen)
        except LoopException as exc:
            exc.validate_solved(self.circles)
            raise SolvedException(
                Board(
                    width=self.width,
                    height=self.height,
                    circles=self.circles,
                    cell_lines=cell_lines,
                    line_segments=[],
                ),
            )

        return Board(
            width=self.width,
            height=self.height,
            circles=self.circles,
            cell_lines=cell_lines,
            line_segments=line_segments,
        )

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

    def disallow_direction(self, x, y, direction):
        old_cell = self.cell_lines[x, y]
        new_cell = old_cell.disallow_direction(direction)
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
        return self.evolve(cell_lines=cell_lookup)

    def __repr__(self):
        return "Board"


class SwappableIterator:
    """
    Overwritable iterator proxy.
    """
    def __init__(self, iterator):
        self._iterator = iter(iterator)

    def __iter__(self):
        return self

    def __next__(self):
        return next(self._iterator)

    def swap(self, iterable):
        """
        Discard the current iterator and start yielding from `iterable` instead.
        """
        self._iterator = iter(iterable)


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
    for direction in cell.could_set():
        with contextlib.suppress(ContradictionException):
            set_black_leg(board, x, y, direction)
            could_dirs.add(direction)

    for direction in could_dirs:
        if direction.opposite() not in could_dirs:
            board = set_black_leg(board, x, y, direction)

    return board


def set_black_leg(board, x, y, direction):
    mx, my = direction.move(x, y)
    board = board.set_direction(x, y, direction)
    return board.set_through(mx, my)


def solve_known_constraints(board):
    old_board = None
    while old_board != board:
        old_board = board
        for (x, y), circle in board.circles.items():
            if circle is WHITE:
                board = apply_white(board, x, y)
            else:
                board = apply_black(board, x, y)
    return board


def _spot_direction_options(board, x, y, direction):
    with contextlib.suppress(ContradictionException):
        yield solve_known_constraints(board.set_direction(x, y, direction))

    with contextlib.suppress(ContradictionException):
        yield solve_known_constraints(board.disallow_direction(x, y, direction))


UNEXPLORED = sentinel.UNEXPLORED


def get_possibility_list(lookahead):  # -> List[PossibilityPair]
    # CURRENT BOARD

    # - yes_board:
    #     - yes_board
    #       no_board
    #   no_board
    # - yes_board:
    #   no_board

    # ---

    # When a possibility pair has one element, collapse its parent list into that element.
    # That is, for `{foo: [{etc:, etc:}, {board: [etc]}], bar: [etc]}`, become
    # `{board: [etc], bar: [etc]}`

    # When a possibility pair has NO elements, raze its parent list.
    # That is, for `{foo: [{etc:, etc:}, {}], bar: [etc]}`, become `{bar: [etc]}`

    # ---

    possibilities = []
    mask = {Direction.Right, Direction.Down}

    board = lookahead.board
    for (x, y), cell in board.cell_lines.items():
        for direction in cell.could_set() & mask:
            boards = list(_spot_direction_options(board, x, y, direction))
            if len(boards) == 1:
                next_board, = boards
                return next_board
            if not boards:
                return []
            possibilities.append(PossibilityPair.new(*boards, parent=lookahead))
    return possibilities


class Ref:
    """oof"""
    def __init__(self, value):
        self.__ref__ = value

    def __getattr__(self, key):
        return getattr(self.__ref__, key)

    def __setattr__(self, key, value):
        if key == '__ref__':
            object.__setattr__(self, key, value)
        else:
            setattr(self.__ref__, key, value)

    def __repr__(self):
        return f"Ref({self.__ref__})"


@attr.s
class Lookahead:
    board = attr.ib()
    possibilities = attr.ib()
    parent = attr.ib()

    @classmethod
    def new(cls, board, parent=None):
        if parent is not None:
            parent = weakref.ref(parent)
        return cls(board, possibilities=UNEXPLORED, parent=parent)

    def get_sibling(self):
        pos = self.parent()
        assert pos
        assert pos.yes.__ref__ == self or pos.no.__ref__ == self
        return pos.no if pos.yes.__ref__ == self else pos.yes


def explore(lookahead_ref):
    queue = collections.deque([lookahead_ref])
    while queue:
        lookahead = queue.popleft()
        if lookahead.possibilities is UNEXPLORED:
            expand(lookahead)
            return True
        else:
            for pos in lookahead.possibilities:
                queue.append(pos.yes)
                queue.append(pos.no)
    return False


def expand(lookahead_ref):
    possibilities = get_possibility_list(lookahead_ref)
    if not possibilities:
        # Contradiction
        if lookahead_ref.parent is None:
            raise ContradictionException("root lookahead encountered contradiction")
        sibling = lookahead_ref.get_sibling()
        parent = lookahead_ref.parent().parent()
        assert parent, "parent was gc'd??"
        sibling.parent = parent.parent
        parent.__ref__ = sibling.__ref__
        if sibling.possibilities is not UNEXPLORED:
            for pos in sibling.possibilities:
                pos.parent = weakref.ref(parent)

    elif isinstance(possibilities, list):
        # Possibilities
        lookahead_ref.possibilities = possibilities
        assert 'possibilities' not in lookahead_ref.__dict__
    else:
        # Certainty
        assert isinstance(possibilities, Board)
        lookahead_ref.board = possibilities


@attr.s
class PossibilityPair:
    yes = attr.ib()
    no = attr.ib()
    parent = attr.ib()

    @classmethod
    def new(cls, yes_board, no_board, *, parent):
        self = cls(None, None, parent=weakref.ref(parent))
        self.yes = Ref(Lookahead.new(yes_board, parent=self))
        self.no = Ref(Lookahead.new(no_board, parent=self))
        return self


def solve(board):
    try:
        root = Ref(Lookahead.new(solve_known_constraints(board)))
        last_seen_board = root.board
        print(print_big_board(root.board))

        while explore(root):
            # print the board if we learned something
            if last_seen_board is not root.board:
                print(print_big_board(root.board))
                last_seen_board = root.board
    except SolvedException as exc:
        return exc.board
    return None


def _validate_lookahead_state(root):
    # print(root)
    # validate the dang state
    nodes = 0
    q = collections.deque([root])
    while q:
        look = q.popleft()
        if look.possibilities is not UNEXPLORED:
            for pos in look.possibilities:
                assert pos.parent() == look, (pos.parent(), look)
                assert pos.yes.parent() == pos, (pos.yes.parent(), pos)
                assert pos.no.parent() == pos, (pos.no.parent(), pos)
                q.append(pos.yes)
                q.append(pos.no)
            nodes += len(look.possibilities)
    print("validated", nodes, "nodes")


def _solve_three_consecutive_whites(board, coord):
    # ooo
    x, y = coord
    if board.circles.get((x + 1, y)) == board.circles.get((x + 2, y)) == WHITE:
        board = board.set_direction(x, y, Direction.Up)
        board = board.set_through(x, y)
        board = board.set_through(x + 1, y)
        board = board.set_through(x + 2, y)
    elif board.circles.get((x, y + 1)) == board.circles.get((x, y + 2)) == WHITE:
        board = board.set_direction(x, y, Direction.Right)
        board = board.set_through(x, y)
        board = board.set_through(x, y + 1)
        board = board.set_through(x, y + 2)
    return board


def _solve_adjacent_blacks(board, coord):
    # ●●
    x, y = coord
    if board.circles.get((x + 1, y)) == BLACK:
        board = set_black_leg(board, x, y, Direction.Left)
        board = set_black_leg(board, x + 1, y, Direction.Right)
    if board.circles.get((x, y + 1)) == BLACK:
        board = set_black_leg(board, x, y, Direction.Up)
        board = set_black_leg(board, x, y + 1, Direction.Down)
    return board


def _solve_overlong_leg(board, coord):
    # ●?oo
    for direction in Direction:
        first_white = direction.move(*direction.move(*coord))
        next_white = direction.move(*first_white)
        if board.circles.get(first_white) == board.circles.get(next_white) == WHITE:
            board = set_black_leg(board, *coord, direction.opposite())
    return board


def _solve_wingman_black(board, coord):
    # ?●?
    # o?o
    for direction in Direction:
        ahead = direction.move(*coord)
        left = direction.turn_left().move(*ahead)
        right = direction.turn_right().move(*ahead)
        if board.circles.get(left) == board.circles.get(right) == WHITE:
            board = set_black_leg(board, *coord, direction.opposite())
    return board


def solve_initial_patterns(board):
    """
    Solve any small, one-time optimizations we can find
    that might be expensive during the main solve.
    """
    for coord, color in board.circles.items():
        if color == WHITE:
            board = _solve_three_consecutive_whites(board, coord)
        else:
            board = _solve_overlong_leg(board, coord)
            board = _solve_adjacent_blacks(board, coord)
            board = _solve_wingman_black(board, coord)
    return board


# display shit


def board_from_string(board_str):
    board_lines = textwrap.dedent(board_str.rstrip().strip('\n')).split('\n')

    return Board(
        width=len(board_lines[0]),
        height=len(board_lines),
        circles=frozendict.frozendict({
            (x, y): SYMBOL_LOOKUP[elem]
            for y, row in enumerate(board_lines)
            for x, elem in enumerate(row)
            if elem != '.'
        }),
    )


def board_from_level(level_name):
    with open(f"../levels/{level_name}.masyu", "r") as file:
        board_str = "".join(line for line in file if not line.startswith("#"))
    return board_from_string(board_str)


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


def main(level_name):
    board = board_from_level(level_name)
    print(print_big_board(board))
    board = solve_initial_patterns(board)
    print(print_big_board(board))
    solved_board = solve(board)
    if solved_board:
        print(print_big_board(solved_board))


if __name__ == '__main__':
    main(sys.argv[1])
