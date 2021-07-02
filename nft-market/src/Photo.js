import AvatarPlaceholder from './AvatarPlaceholder';
import './Photo.css';

const imgWithClick = { cursor: 'pointer' };

const Photo = ({
  index,
  onClick,
  photo,
  buy,
  margin,
  direction,
  top,
  left,
  key
}) => {
  const imgStyle = { margin: margin, display: 'block' };
  const { src, width, height, seller, price, title, earthPrice, tokenId } =
    photo;
  if (direction === 'column') {
    imgStyle.position = 'absolute';
    imgStyle.left = left;
    imgStyle.top = top;
    imgStyle.width = width;
    imgStyle.height = height;
  }

  const handleClick = (event) => {
    onClick(event, { photo, index });
  };

  return (
    <div
      key={tokenId}
      className="nft-item"
      style={onClick ? { ...imgStyle, ...imgWithClick } : imgStyle}
      onClick={onClick ? handleClick : null}
    >
      <img className="nft-image" alt={key} src={src} />
      <div className="nft-bottom">
        <AvatarPlaceholder className="nft-avatar" address={seller} />
        {buy && seller}
        <span className="nft-title">
          {title}
          <br />
          <span className="nft-price">{price} mars</span>
          &nbsp;&nbsp;<span style={{ color: '#F68A1D' }}>or</span>&nbsp;&nbsp;
          <span className="nft-price">{earthPrice} earth</span>
        </span>
        {buy && (
          <button
            onClick={buy}
            style={{ padding: '10px 20px', marginRight: 10 }}
          >
            Buy
          </button>
        )}
      </div>
    </div>
  );
};

export default Photo;
