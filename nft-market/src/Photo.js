import AvatarPlaceholder from './AvatarPlaceholder';
import './Photo.css';

const imgWithClick = { cursor: 'pointer' };

const Photo = ({
  index,
  onClick,
  photo,
  margin,
  direction,
  top,
  left,
  key
}) => {
  const imgStyle = { margin: margin, display: 'block' };
  const { src, width, height, owner, price, title } = photo;
  console.log(photo);
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
      key={key}
      className="nft-item"
      style={onClick ? { ...imgStyle, ...imgWithClick } : imgStyle}
      onClick={onClick ? handleClick : null}
    >
      <img className="nft-image" alt={key} src={src} />
      <div className="nft-bottom">
        <AvatarPlaceholder className="nft-avatar" address={owner} />
        <span className="nft-title">
          {title}
          <br />
          <span className="nft-price">{price}</span>
        </span>
      </div>
    </div>
  );
};

export default Photo;
